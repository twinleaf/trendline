use super::decimation::StreamingFpcsPipeline;
use super::detrend::DetrendPipeline;
use super::fft::FftPipeline;
use super::passthrough::PassthroughPipeline;
use super::Pipeline;
use crate::pipeline::statistics::StreamingStatisticsProvider;
use crate::pipeline::StatisticsProvider;
use crate::shared::{
    DataColumnId, DecimationMethod, DetrendMethod, FftConfig, PipelineId, PlotData,
    SharedPlotConfig, TimeseriesConfig, ViewConfig,
};
use crate::state::capture::CaptureState;
use crate::util::k_way_merge_plot_data;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};

pub struct ManagedPlotPipeline {
    config: SharedPlotConfig,
    output_pipeline_ids: Vec<PipelineId>,
    all_component_ids: Vec<PipelineId>,
}

pub struct ProcessingManager {
    pub managed_plots: HashMap<String, ManagedPlotPipeline>,
    pub pipelines: HashMap<PipelineId, Arc<Mutex<dyn Pipeline>>>,
    pub stat_providers: HashMap<PipelineId, Arc<Mutex<dyn StatisticsProvider>>>,
    pub plot_channels: HashMap<String, Channel>,
    pub statistics_channels: HashMap<PipelineId, Channel>,
}

impl ProcessingManager {
    pub fn new_with_ticker(capture_state: CaptureState) -> Arc<Mutex<Self>> {
        let manager = Arc::new(Mutex::new(Self {
            managed_plots: HashMap::new(),
            pipelines: HashMap::new(),
            stat_providers: HashMap::new(),
            plot_channels: HashMap::new(),
            statistics_channels: HashMap::new(),
        }));

        let manager_clone = manager.clone();

        thread::Builder::new()
            .name("pipeline-ticker".into())
            .spawn(move || {
                loop {
                    let (pipelines_to_update, providers_to_update) = {
                        let Ok(mg) = manager_clone.lock() else {
                            eprintln!("[Ticker] Manager lock poisoned, exiting thread.");
                            break;
                        };
                        let p_arcs: Vec<_> = mg.pipelines.values().cloned().collect();
                        let s_arcs: Vec<_> = mg.stat_providers.values().cloned().collect();
                        (p_arcs, s_arcs)
                    };

                    pipelines_to_update
                        .par_iter()
                        .for_each(|pipeline_mutex| {
                            if let Ok(mut pipeline) = pipeline_mutex.lock() {
                                pipeline.update(&capture_state);
                            }
                        });
                    
                    providers_to_update
                        .par_iter()
                        .for_each(|provider_mutex| {
                            if let Ok(mut provider) = provider_mutex.lock() {
                                provider.update(&capture_state);
                            }
                        });
                    

                    if let Ok(mg) = manager_clone.lock() {
                        let channels_to_push: Vec<_> = mg.plot_channels.iter()
                            .map(|(id, chan)| (id.clone(), chan.clone()))
                            .collect();

                        for (plot_id, channel) in channels_to_push {
                            let plot_data = mg.get_merged_data_for_plot(&plot_id);

                            if !plot_data.is_empty() {
                                match serde_json::to_string(&plot_data) {
                                    Ok(json_string) => {
                                        let response_body = InvokeResponseBody::Json(json_string);
                                        if let Err(e) = channel.send(response_body) {
                                            eprintln!("[Ticker] Failed to send on channel for plot {}: {}", plot_id, e);
                                        }
                                    },
                                    Err(e) => {
                                        eprintln!("[Ticker] CRITICAL: Failed to serialize PlotData for plot {}: {}", plot_id, e);
                                    }
                                }
                            }
                            else {
                                println!("Publishing zero length data...");
                            }
                        }

                        let stats_channels_to_push: Vec<_> = mg.statistics_channels.iter()
                            .map(|(id, chan)| (*id, chan.clone()))
                            .collect();
                        for (provider_id, channel) in stats_channels_to_push {
                             if let Some(provider_mutex) = mg.stat_providers.get(&provider_id) {
                                let stats_output = provider_mutex.lock().unwrap().get_output();
                                match serde_json::to_string(&stats_output) {
                                    Ok(json_string) => {
                                        let response_body = InvokeResponseBody::Json(json_string);
                                        if let Err(e) = channel.send(response_body) {
                                            eprintln!("[Ticker] Failed to send on channel for stats provider {}: {}", provider_id.0, e);
                                        }
                                    },
                                    Err(e) => {
                                        eprintln!("[Ticker] CRITICAL: Failed to serialize StreamStatistics for provider {}: {}", provider_id.0, e);
                                    }
                                }
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(33));
                }
            })
            .expect("Failed to spawn pipeline ticker thread.");

        manager
    }
    pub fn register_statistics_channel(&mut self, provider_id: PipelineId, channel: Channel) {
        println!(
            "[Manager] Registering IPC channel for statistics provider ID: {}",
            provider_id.0
        );
        self.statistics_channels.insert(provider_id, channel);
    }

    pub fn register_plot_channel(&mut self, plot_id: String, channel: Channel) {
        println!("[Manager] Registering IPC channel for plot ID: {}", plot_id);
        self.plot_channels.insert(plot_id, channel);
    }

    pub fn create_fpcs_pipeline(
        &mut self,
        source_key: DataColumnId,
        ratio: usize,
        window_seconds: f64,
    ) -> PipelineId {
        let pipeline = StreamingFpcsPipeline::new(source_key, ratio, window_seconds);
        let id = pipeline.id();
        println!("[Pipeline] Creating FPCS pipeline. ID: {:?}", id);
        self.pipelines.insert(id, Arc::new(Mutex::new(pipeline)));
        id
    }

    pub fn create_passthrough_pipeline(
        &mut self,
        source_key: DataColumnId,
        window_seconds: f64,
    ) -> PipelineId {
        let pipeline = PassthroughPipeline::new(source_key, window_seconds);
        let id = pipeline.id();
        println!("[Pipeline] Creating passthrough pipeline. ID: {:?}", id);
        self.pipelines.insert(id, Arc::new(Mutex::new(pipeline)));
        id
    }

    pub fn create_detrend_pipeline(
        &mut self,
        source_key: DataColumnId,
        window_seconds: f64,
        method: DetrendMethod,
    ) -> PipelineId {
        let pipeline = DetrendPipeline::new(source_key, window_seconds, method);
        let id = pipeline.id();
        println!("[Pipeline] Creating detrend pipeline. ID: {:?}", id);
        self.pipelines.insert(id, Arc::new(Mutex::new(pipeline)));
        id
    }

    pub fn create_fft_pipeline_from_source(
        &mut self,
        source_pipeline_id: PipelineId,
    ) -> Result<PipelineId, String> {
        let source_pipeline = self
            .pipelines
            .get(&source_pipeline_id)
            .ok_or_else(|| "Source pipeline not found".to_string())?
            .clone();

        let fft_pipeline = FftPipeline::new(source_pipeline);
        let new_id = fft_pipeline.id();
        println!(
            "[Pipeline] Creating FFT pipeline. ID: {:?}, Source ID: {:?}",
            new_id, source_pipeline_id
        );
        self.pipelines
            .insert(new_id, Arc::new(Mutex::new(fft_pipeline)));
        Ok(new_id)
    }
    pub fn apply_plot_config(
        &mut self,
        config: SharedPlotConfig,
    ) -> Result<Vec<PipelineId>, String> {
        println!("[Manager] Applying config for plot ID: {}", config.plot_id);

        if let Some(existing_plot) = self.managed_plots.get(&config.plot_id) {
            if existing_plot.config.view_config == config.view_config
                && existing_plot.config.data_keys == config.data_keys
            {
                println!(
                    "[Manager] Config for {} is unchanged. No action needed.",
                    config.plot_id
                );
                return Ok(existing_plot.output_pipeline_ids.clone());
            }

            println!(
                "[Manager] Config for {} changed. Rebuilding.",
                config.plot_id
            );
            self._destroy_plot_components(&config.plot_id);
        }

        let mut output_pipeline_ids = Vec::new();
        let mut all_component_ids = Vec::new();

        for key in &config.data_keys {
            match &config.view_config {
                ViewConfig::Timeseries(ts_config) => {
                    let id =
                        self._create_timeseries_for_plot(key, ts_config, config.max_sampling_rate);
                    output_pipeline_ids.push(id);
                    all_component_ids.push(id);
                }
                ViewConfig::Fft(fft_config) => {
                    let (fft_id, source_id) = self._create_fft_chain_for_plot(key, fft_config)?;
                    output_pipeline_ids.push(fft_id);
                    all_component_ids.push(source_id);
                    all_component_ids.push(fft_id);
                }
            }
        }

        let managed_plot = ManagedPlotPipeline {
            config: config.clone(),
            output_pipeline_ids: output_pipeline_ids.clone(),
            all_component_ids,
        };
        self.managed_plots.insert(config.plot_id, managed_plot);

        Ok(output_pipeline_ids)
    }

    pub fn destroy_plot_pipelines(&mut self, plot_id: &str) {
        self._destroy_plot_components(plot_id);
        if self.plot_channels.remove(plot_id).is_some() {
            println!(
                "[Manager] Completely removed plot and IPC channel for ID: {}",
                plot_id
            );
        }
    }

    fn _create_timeseries_for_plot(
        &mut self,
        source_key: &DataColumnId,
        config: &TimeseriesConfig,
        max_sr: f64,
    ) -> PipelineId {
        match config.decimation_method {
            DecimationMethod::Fpcs => {
                let total_points_in_window = max_sr * config.window_seconds;
                let target_points = 10.0 * config.resolution_multiplier as f64;
                let ratio = (total_points_in_window / target_points).round() as usize;
                self.create_fpcs_pipeline(source_key.clone(), ratio.max(1), config.window_seconds)
            }
            DecimationMethod::None => {
                self.create_passthrough_pipeline(source_key.clone(), config.window_seconds)
            }
        }
    }

    fn _create_fft_chain_for_plot(
        &mut self,
        source_key: &DataColumnId,
        config: &FftConfig,
    ) -> Result<(PipelineId, PipelineId), String> {
        let intermediate_id = match config.detrend_method {
            DetrendMethod::None => {
                self.create_passthrough_pipeline(source_key.clone(), config.window_seconds)
            }
            _ => self.create_detrend_pipeline(
                source_key.clone(),
                config.window_seconds,
                config.detrend_method.clone(),
            ),
        };

        let fft_id = self.create_fft_pipeline_from_source(intermediate_id)?;
        Ok((fft_id, intermediate_id))
    }

    fn _destroy_plot_components(&mut self, plot_id: &str) {
        if let Some(plot_to_remove) = self.managed_plots.remove(plot_id) {
            println!(
                "[Manager] Destroying component pipelines for plot ID: {}",
                plot_id
            );
            for id in plot_to_remove.all_component_ids {
                self.destroy(id);
            }
        }
    }

    pub fn get_merged_data_for_plot(&self, plot_id: &str) -> PlotData {
        let Some(managed_plot) = self.managed_plots.get(plot_id) else {
            return PlotData::empty();
        };

        let mut plot_data_to_merge = Vec::with_capacity(managed_plot.output_pipeline_ids.len());
        for id in &managed_plot.output_pipeline_ids {
            if let Some(pipeline_mutex) = self.pipelines.get(id) {
                if let Ok(pipeline) = pipeline_mutex.try_lock() {
                    plot_data_to_merge.push(pipeline.get_output().clone());
                }
            }
        }

        if plot_data_to_merge.is_empty() {
            return PlotData::empty();
        }

        k_way_merge_plot_data(plot_data_to_merge)
    }

    pub fn create_statistics_provider(
        &mut self,
        source_key: DataColumnId,
        window_seconds: f64,
    ) -> PipelineId {
        let provider = StreamingStatisticsProvider::new(source_key, window_seconds);
        let id = provider.id();
        println!("[Pipeline] Creating statistics provider. ID: {:?}", id);
        self.stat_providers
            .insert(id, Arc::new(Mutex::new(provider)));
        id
    }

    pub fn destroy(&mut self, id: PipelineId) {
        println!("[Pipeline] Destroying processor. ID: {:?}", id);
        self.pipelines.remove(&id);
        self.stat_providers.remove(&id);
        self.statistics_channels.remove(&id);
    }
}
