use super::decimation::StreamingFpcsPipeline;
use super::detrend::DetrendPipeline;
use super::fft::FftPipeline;
use super::passthrough::PassthroughPipeline;
use super::{Pipeline, PipelineCommand};
use crate::pipeline::statistics::StreamingStatisticsProvider;
use crate::pipeline::StatisticsProvider;
use crate::shared::{
    DataColumnId, DecimationMethod, FftConfig, PipelineId, PlotData, SharedPlotConfig,
    StreamStatistics, TimeseriesConfig, ViewConfig,
};
use crate::state::capture::{CaptureCommand, CaptureState};
use crate::util::k_way_merge_plot_data;
use crossbeam::channel::{bounded, select, Sender};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::ipc::Channel;

pub struct ManagedPlotPipeline {
    pub config: SharedPlotConfig,
    pub(crate) output_pipeline_ids: Vec<PipelineId>,
    pub(crate) all_component_ids: Vec<PipelineId>,
}

enum ThreadType {
    Root {
        sub_id: usize,
        source_key: DataColumnId,
    },
    Statistics {
        sub_id: usize,
        source_key: DataColumnId,
    },
    Derived,
}


struct PipelineThreadHandle {
    cmd_tx: Sender<PipelineCommand>,
    handle: JoinHandle<()>,
    thread_type: ThreadType,
}

pub struct ProcessingManager {
    pub managed_plots: HashMap<String, ManagedPlotPipeline>,
    pub pipelines: HashMap<PipelineId, Arc<Mutex<dyn Pipeline>>>,
    pub stat_providers: HashMap<PipelineId, Arc<Mutex<dyn StatisticsProvider>>>,
    pub plot_channels: HashMap<String, Channel<PlotData>>,
    pub statistics_channels: HashMap<PipelineId, Channel<StreamStatistics>>,
    pipeline_threads: HashMap<PipelineId, PipelineThreadHandle>,
    capture_state: CaptureState,
    capture_cmd_tx: Sender<CaptureCommand>,
    next_sub_id: AtomicUsize,
}

impl ProcessingManager {
    pub fn new_with_ticker(capture_state: CaptureState) -> Arc<Mutex<Self>> {
        let capture_cmd_tx = capture_state.inner.command_tx.clone();
        let manager = Arc::new(Mutex::new(Self {
            managed_plots: HashMap::new(),
            pipelines: HashMap::new(),
            stat_providers: HashMap::new(),
            plot_channels: HashMap::new(),
            statistics_channels: HashMap::new(),
            pipeline_threads: HashMap::new(),
            capture_state,
            capture_cmd_tx,
            next_sub_id: AtomicUsize::new(1),
        }));

        {
            let manager_clone = manager.clone();
            thread::Builder::new()
                .name("ui-emitter".into())
                .spawn(move || loop {
                    thread::sleep(Duration::from_millis(33));
                    if let Ok(mg) = manager_clone.lock() {
                        // Plot channel logic (remains the same)
                        for (plot_id, channel) in &mg.plot_channels {
                            if let Some(data) = mg.get_merged_data_for_plot(plot_id) {
                                if !data.is_empty() {
                                    let _ = channel.send(data);
                                }
                            }
                        }
                        
                        for (provider_id, channel) in &mg.statistics_channels {
                            if let Some(provider) = mg.stat_providers.get(provider_id) {
                                let mut provider_locked = provider.lock().unwrap();
                                let stats = provider_locked.get_output(&mg.capture_state);
                                let _ = channel.send(stats);
                            }
                        }
                    } else {
                        break;
                    }
                })
                .expect("Failed to spawn ui-emitter thread");
        }

        manager
    }

    fn spawn_root_pipeline<P>(&mut self, pipeline: P, source_key: DataColumnId) -> PipelineId
    where
        P: Pipeline + 'static,
    {
        let id = pipeline.id();
        let pipeline_arc = Arc::new(Mutex::new(pipeline));
        self.pipelines.insert(id, pipeline_arc.clone());
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        let (data_tx, data_rx) = bounded(128);
        let (cmd_tx, cmd_rx) = bounded(16);
        self.capture_cmd_tx
            .send(CaptureCommand::Subscribe {
                key: source_key.clone(),
                id: sub_id,
                tx: data_tx,
            })
            .unwrap();
        let _ = cmd_tx.send(PipelineCommand::Hydrate);
        let capture_clone = self.capture_state.clone();
        let handle = thread::Builder::new()
            .name(format!("pipeline-root-{:?}", id))
            .spawn(move || loop {
                select! {
                    recv(data_rx) -> msg => match msg {
                        Ok(batch) => pipeline_arc.lock().unwrap().process_batch(batch),
                        Err(_) => break,
                    },
                    recv(cmd_rx) -> msg => match msg {
                        Ok(cmd) => {
                            if matches!(cmd, PipelineCommand::Shutdown) { break; }
                            pipeline_arc.lock().unwrap().process_command(cmd, &capture_clone);
                        },
                        Err(_) => break,
                    }
                }
            })
            .unwrap();
        self.pipeline_threads.insert(id, PipelineThreadHandle {
            cmd_tx,
            handle,
            thread_type: ThreadType::Root { sub_id, source_key },
        });
        id
    }

    fn spawn_derived_pipeline<P>(
        &mut self,
        pipeline: P,
    ) -> (PipelineId, Sender<(PlotData, f64)>)
    where
        P: Pipeline + 'static,
    {
        let id = pipeline.id();
        let pipeline_arc = Arc::new(Mutex::new(pipeline));
        self.pipelines.insert(id, pipeline_arc.clone());

        let (data_tx, data_rx) = bounded(1);
        let (cmd_tx, cmd_rx) = bounded(16);
        let capture_clone = self.capture_state.clone();
        let handle = thread::Builder::new()
            .name(format!("pipeline-derived-{:?}", id))
            .spawn(move || loop {
                select! {
                    // This will now block until the single slot in the channel is free.
                    recv(data_rx) -> msg => match msg {
                        Ok(batch) => pipeline_arc.lock().unwrap().process_derived_batch(batch),
                        Err(_) => break,
                    },
                    recv(cmd_rx) -> msg => match msg {
                        Ok(cmd) => {
                            if matches!(cmd, PipelineCommand::Shutdown) { break; }
                            pipeline_arc.lock().unwrap().process_command(cmd, &capture_clone);
                        },
                        Err(_) => break,
                    }
                }
            })
            .unwrap();
        self.pipeline_threads.insert(
            id,
            PipelineThreadHandle {
                cmd_tx,
                handle,
                thread_type: ThreadType::Derived,
            },
        );
        (id, data_tx)
    }

    pub fn apply_plot_config(
        &mut self,
        config: SharedPlotConfig,
    ) -> Result<Vec<PipelineId>, String> {
        if self.managed_plots.contains_key(&config.plot_id) {
            self._destroy_plot_components(&config.plot_id);
        }
        let mut output_ids = Vec::new();
        let mut all_ids = Vec::new();
        for key in &config.data_keys {
            match &config.view_config {
                ViewConfig::Timeseries(ts_config) => {
                    let id =
                        self._create_timeseries_for_plot(key, ts_config, config.max_sampling_rate);
                    output_ids.push(id);
                    all_ids.push(id);
                }
                ViewConfig::Fft(fft_config) => {
                    let (fft_id, src_id) = self._create_fft_chain_for_plot(key, fft_config)?;
                    output_ids.push(fft_id);
                    all_ids.push(src_id);
                    all_ids.push(fft_id);
                }
            }
        }
        self.managed_plots.insert(
            config.plot_id.clone(),
            ManagedPlotPipeline {
                config,
                output_pipeline_ids: output_ids.clone(),
                all_component_ids: all_ids,
            },
        );
        Ok(output_ids)
    }

    pub fn destroy_plot_pipelines(&mut self, plot_id: &str) {
        self._destroy_plot_components(plot_id);
        self.plot_channels.remove(plot_id);
    }

    fn _create_timeseries_for_plot(
        &mut self,
        key: &DataColumnId,
        config: &TimeseriesConfig,
        max_sr: f64,
    ) -> PipelineId {
        match config.decimation_method {
            DecimationMethod::Fpcs => {
                let ratio = ((max_sr * config.window_seconds)
                    / (10.0 * config.resolution_multiplier as f64))
                    .round() as usize;
                let pipeline = StreamingFpcsPipeline::new(key.clone(), ratio.max(1), config.window_seconds);
                self.spawn_root_pipeline(pipeline, key.clone())
            }
            DecimationMethod::None => {
                let pipeline = PassthroughPipeline::new(key.clone(), config.window_seconds);
                self.spawn_root_pipeline(pipeline, key.clone())
            }
        }
    }

    fn _create_fft_chain_for_plot(
        &mut self,
        key: &DataColumnId,
        config: &FftConfig,
    ) -> Result<(PipelineId, PipelineId), String> {
        let detrend_pipeline =
            DetrendPipeline::new(key.clone(), config.window_seconds, config.detrend_method.clone());
        let detrend_id = self.spawn_root_pipeline(detrend_pipeline, key.clone());
        
        let fft_pipeline = FftPipeline::new();
        let (fft_id, fft_input_tx) = self.spawn_derived_pipeline(fft_pipeline);
        
        let handle = self.pipeline_threads.get(&detrend_id).ok_or("Detrend handle not found")?;
        
        handle.cmd_tx.send(PipelineCommand::AddSubscriber(fft_input_tx)).map_err(|e| e.to_string())?;
        
        Ok((fft_id, detrend_id))
    }

    fn _destroy_plot_components(&mut self, plot_id: &str) {
        if let Some(plot) = self.managed_plots.remove(plot_id) {
            for id in plot.all_component_ids.iter().rev() {
                self.destroy(*id);
            }
        }
    }

    pub fn get_merged_data_for_plot(&self, plot_id: &str) -> Option<PlotData> {
        let managed_plot = self.managed_plots.get(plot_id)?;
        let mut data_to_merge = Vec::with_capacity(managed_plot.output_pipeline_ids.len());
        for id in &managed_plot.output_pipeline_ids {
            if let Some(p) = self.pipelines.get(id) {
                if let Ok(pipeline) = p.try_lock() {
                    data_to_merge.push(pipeline.get_output());
                }
            }
        }
        if data_to_merge.is_empty() {
            None
        } else {
            Some(k_way_merge_plot_data(data_to_merge))
        }
    }

    pub fn create_statistics_provider(
        &mut self,
        source_key: DataColumnId,
        window_seconds: f64,
    ) -> PipelineId {
        let provider = StreamingStatisticsProvider::new(source_key.clone(), window_seconds);
        let id = provider.id();
        let provider_arc = Arc::new(Mutex::new(provider));
        self.stat_providers.insert(id, provider_arc.clone());

        let (cmd_tx, cmd_rx) = bounded(16);
        let (data_tx, data_rx) = bounded(128);

        let sub_id = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        println!(
            "[Stats] Creating provider {:?} for stream (url={}, route={}, stream={}, col={}, window={:.2}s); sub_id={}",
            id, source_key.port_url, source_key.device_route, source_key.stream_id, source_key.column_index, window_seconds, sub_id
        );

        self.capture_cmd_tx
            .send(CaptureCommand::Subscribe { key: source_key.clone(), id: sub_id, tx: data_tx })
            .unwrap();

        let handle = thread::Builder::new()
            .name(format!("pipeline-stats-{:?}", id))
            .spawn(move || {
                println!("[Stats {:?}] thread started (sub_id={}).", id, sub_id);
                loop {
                    select! {
                        recv(data_rx) -> msg => match msg {
                            Ok(batch) => provider_arc.lock().unwrap().process_batch(batch),
                            Err(_) => break, // channel closed
                        },
                        recv(cmd_rx) -> msg => match msg {
                            Ok(PipelineCommand::Shutdown) => break,
                            Ok(_) => {},
                            Err(_) => break,
                        }
                    }
                }
                println!("[Stats {:?}] thread exiting.", id);
            })
            .unwrap();

        self.pipeline_threads.insert(
            id,
            PipelineThreadHandle {
                cmd_tx,
                handle,
                thread_type: ThreadType::Statistics { sub_id, source_key: source_key.clone() },
            },
        );

        id
    }

    pub fn destroy(&mut self, id: PipelineId) {
        if let Some(handle) = self.pipeline_threads.remove(&id) {
            match &handle.thread_type {
                ThreadType::Statistics { sub_id, source_key } => {
                    println!("[Pipeline] Destroying stats provider {:?} (unsub sub_id={} stream url={} route={} sid={}).",
                        id, sub_id, source_key.port_url, source_key.device_route, source_key.stream_id);
                    let _ = self.capture_cmd_tx.send(CaptureCommand::Unsubscribe {
                        key: source_key.clone(),
                        id: *sub_id,
                    });
                }
                ThreadType::Root { sub_id, source_key } => {
                    println!("[Pipeline] Destroying root pipeline {:?} (unsub sub_id={}, stream sid={}).",
                        id, sub_id, source_key.stream_id);
                    let _ = self.capture_cmd_tx.send(CaptureCommand::Unsubscribe {
                        key: source_key.clone(),
                        id: *sub_id,
                    });
                }
                ThreadType::Derived => {
                    println!("[Pipeline] Destroying derived pipeline {:?}.", id);
                }
            }

            let _ = handle.cmd_tx.send(PipelineCommand::Shutdown);
            if let Err(e) = handle.handle.join() {
                eprintln!("[Pipeline] Join failed for {:?}: {:?}", id, e);
            } else {
                println!("[Pipeline] Cleaned up thread for {:?}.", id);
            }
        }

        let removed_p = self.pipelines.remove(&id).is_some();
        let removed_s = self.stat_providers.remove(&id).is_some();
        let removed_c = self.statistics_channels.remove(&id).is_some();
        if removed_p || removed_s || removed_c {
            println!("[Pipeline] Removed maps for {:?} (pipelines={}, stats={}, chan={}).",
                    id, removed_p, removed_s, removed_c);
        }
    }
        
    pub fn register_plot_channel(&mut self, plot_id: String, channel: Channel<PlotData>) {
        self.plot_channels.insert(plot_id, channel);
    }

    pub fn register_statistics_channel(
        &mut self,
        provider_id: PipelineId,
        channel: Channel<StreamStatistics>,
    ) {
        println!("[Manager] Registering IPC channel for stats provider {:?}", provider_id);
        self.statistics_channels.insert(provider_id, channel);
    }
}