use super::decimation::StreamingFpcsPipeline;
use super::detrend::DetrendPipeline;
use super::fft::FftPipeline;
use super::passthrough::PassthroughPipeline;
use super::{Pipeline, PipelineCommand};
use crate::pipeline::arithmetic::ArithmeticProducer;
use crate::pipeline::filtering::BiquadProducer;
use crate::pipeline::statistics::StreamingStatisticsProvider;
use crate::pipeline::StatisticsProvider;
use crate::shared::Point;
use crate::shared::{
    ColumnStatistics, DecimationMethod, FftConfig, MathChannelConfig, PipelineId, PlotData, PlotId,
    PlotViewType, TimeseriesConfig, UiColumnKey, UiPlotState, ViewConfig, VIRTUAL_ROUTE,
};
use crate::state::data_manager::{BatchedData, DataMsg};
use crate::util::k_way_merge_plot_data;
use crossbeam::channel::{bounded, select, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use uuid::Uuid;

static NEXT_MATH_STREAM_ID: AtomicU8 = AtomicU8::new(1);

pub enum PipelineMsg {
    // Plot lifecycle
    CreatePlot {
        reply: Sender<UiPlotState>,
    },
    DestroyPlot {
        plot_id: PlotId,
    },
    RegisterPlotChannel {
        plot_id: PlotId,
        channel: Channel<PlotData>,
    },

    // Statistics
    CreateStatisticsProvider {
        source_key: UiColumnKey,
        window_seconds: f64,
        reply: Sender<PipelineId>,
    },
    RegisterStatisticsChannel {
        provider_id: PipelineId,
        channel: Channel<ColumnStatistics>,
    },

    // Math channels
    CreateMathChannel {
        spec: MathChannelConfig,
        input_rates: HashMap<UiColumnKey, Option<f64>>,
        reply: Sender<Result<MathChannelResult, String>>,
    },
    DestroyMathChannel {
        output_key: UiColumnKey,
    },

    // Destruction / reset
    DestroyProcessor {
        id: PipelineId,
    },
    ResetPlot {
        plot_id: PlotId,
        reply: Sender<Result<(), String>>,
    },
    ResetByPipelineId {
        id: PipelineId,
        reply: Sender<Result<(), String>>,
    },
    ResetByColumn {
        key: UiColumnKey,
        reply: Sender<Result<(), String>>,
    },
    ResetByStream {
        key: UiColumnKey,
        reply: Sender<Result<(), String>>,
    },
    ResetByDevice {
        key: UiColumnKey,
        reply: Sender<Result<(), String>>,
    },
    ResetAllRuntimeState,

    // Individual config updates
    UpdatePlotColumns {
        plot_id: PlotId,
        data_keys: Vec<UiColumnKey>,
        max_sampling_rate: f64,
        view_config: ViewConfig,
        rates: HashMap<UiColumnKey, f64>,
        backfills: HashMap<UiColumnKey, Vec<Point>>,
        reply: Sender<Result<UiPlotState, String>>,
    },
    UpdatePlotViewConfig {
        plot_id: PlotId,
        view_config: ViewConfig,
        backfills: HashMap<UiColumnKey, Vec<Point>>,
        reply: Sender<Result<UiPlotState, String>>,
    },

    // Rate propagation
    SamplingRatesChanged {
        rates: HashMap<UiColumnKey, f64>,
    },

    // Query
    GetPlotDataKeys {
        plot_id: PlotId,
        reply: Sender<Option<Vec<UiColumnKey>>>,
    },

    Shutdown,
}

pub struct MathChannelResult {
    pub pipeline_id: PipelineId,
    pub output_key: UiColumnKey,
    pub output_rate: Option<f64>,
}

pub struct PipelineManager {
    tx: Sender<PipelineMsg>,
}

impl PipelineManager {
    pub fn new(data_sub_tx: Sender<DataMsg>) -> Self {
        let (tx, rx) = unbounded::<PipelineMsg>();
        let model = PipelineModel::new(data_sub_tx);
        Self::spawn_actor(model, rx);
        Self { tx }
    }

    pub fn sender(&self) -> Sender<PipelineMsg> {
        self.tx.clone()
    }

    pub fn dispatch(&self, msg: PipelineMsg) -> Result<(), String> {
        self.tx
            .send(msg)
            .map_err(|_| "PipelineManager actor is unavailable.".to_string())
    }

    pub fn ask<R>(&self, build: impl FnOnce(Sender<R>) -> PipelineMsg) -> Result<R, String>
    where
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = bounded(1);
        self.tx
            .send(build(reply_tx))
            .map_err(|_| "PipelineManager actor is unavailable.".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "PipelineManager actor did not respond.".to_string())
    }

    fn spawn_actor(model: PipelineModel, rx: Receiver<PipelineMsg>) {
        thread::Builder::new()
            .name("pipeline-manager".into())
            .spawn(move || {
                let mut model = model;
                let plot_period = Duration::from_millis(33);
                let stats_period = Duration::from_millis(16);
                let mut next_plot = Instant::now();
                let mut next_stats = Instant::now();
                let virtual_rx = model.virtual_data_rx.clone();

                loop {
                    let timeout =
                        (next_plot.min(next_stats)).saturating_duration_since(Instant::now());
                    select! {
                        recv(rx) -> msg => match msg {
                            Ok(PipelineMsg::Shutdown) => break,
                            Ok(msg) => model.handle_msg(msg),
                            Err(_) => break,
                        },
                        recv(virtual_rx) -> batch => {
                            if let Ok(batch) = batch {
                                model.fan_out_virtual(batch);
                            }
                        },
                        default(timeout) => { /* fall through to tick */ }
                    }

                    let now = Instant::now();
                    if now >= next_plot {
                        model.tick_plot_channels();
                        next_plot = now + plot_period;
                    }
                    if now >= next_stats {
                        model.tick_statistics_channels();
                        next_stats = now + stats_period;
                    }
                }
            })
            .expect("Failed to spawn pipeline-manager thread");
    }
}

pub struct ManagedPlotPipeline {
    pub config: UiPlotState,
    pub(crate) output_pipeline_ids: Vec<PipelineId>,
    pub(crate) all_component_ids: Vec<PipelineId>,
}

enum ThreadType {
    Root {
        sub_id: usize,
        source_key: UiColumnKey,
    },
    Statistics {
        sub_id: usize,
        source_key: UiColumnKey,
    },
    Derived {
        source_key: UiColumnKey,
    },
    ProducerMono {
        sub_id: usize,
        source_key: UiColumnKey,
        output_key: UiColumnKey,
    },
    ProducerBi {
        sub_id_a: usize,
        sub_id_b: usize,
        source_a: UiColumnKey,
        source_b: UiColumnKey,
        output_key: UiColumnKey,
    },
}

struct PipelineThreadHandle {
    cmd_tx: Sender<PipelineCommand>,
    handle: JoinHandle<()>,
    thread_type: ThreadType,
}

struct PipelineModel {
    managed_plots: HashMap<PlotId, ManagedPlotPipeline>,
    pipelines: HashMap<PipelineId, Arc<Mutex<dyn Pipeline>>>,
    stat_providers: HashMap<PipelineId, Arc<Mutex<dyn StatisticsProvider>>>,
    plot_channels: HashMap<PlotId, Channel<PlotData>>,
    statistics_channels: HashMap<PipelineId, Channel<ColumnStatistics>>,
    pipeline_threads: HashMap<PipelineId, PipelineThreadHandle>,
    data_sub_tx: Sender<DataMsg>,
    cached_rates: HashMap<UiColumnKey, f64>,
    virtual_data_tx: Sender<Arc<BatchedData>>,
    virtual_data_rx: Receiver<Arc<BatchedData>>,
    virtual_subscribers: HashMap<UiColumnKey, Vec<(usize, Sender<Arc<BatchedData>>)>>,
    virtual_streams: HashMap<UiColumnKey, PipelineId>,
    next_sub_id: AtomicUsize,
}

impl PipelineModel {
    fn new(data_sub_tx: Sender<DataMsg>) -> Self {
        let (virtual_data_tx, virtual_data_rx) = bounded(256);
        Self {
            managed_plots: HashMap::new(),
            pipelines: HashMap::new(),
            stat_providers: HashMap::new(),
            plot_channels: HashMap::new(),
            statistics_channels: HashMap::new(),
            pipeline_threads: HashMap::new(),
            data_sub_tx,
            cached_rates: HashMap::new(),
            virtual_data_tx,
            virtual_data_rx,
            virtual_subscribers: HashMap::new(),
            virtual_streams: HashMap::new(),
            next_sub_id: AtomicUsize::new(1),
        }
    }

    fn fan_out_virtual(&self, batch: Arc<BatchedData>) {
        if let Some(subs) = self.virtual_subscribers.get(&batch.key) {
            for (_, tx) in subs {
                let _ = tx.try_send(batch.clone());
            }
        }
    }

    fn subscribe_mut(&mut self, key: &UiColumnKey, sub_id: usize, tx: Sender<Arc<BatchedData>>) {
        if key.is_virtual() {
            self.virtual_subscribers
                .entry(key.clone())
                .or_default()
                .push((sub_id, tx));
        } else {
            let _ = self.data_sub_tx.send(DataMsg::Subscribe {
                key: key.clone(),
                id: sub_id,
                tx,
            });
        }
    }

    fn unsubscribe(&mut self, key: &UiColumnKey, sub_id: usize) {
        if key.is_virtual() {
            if let Some(subs) = self.virtual_subscribers.get_mut(key) {
                subs.retain(|(id, _)| *id != sub_id);
                if subs.is_empty() {
                    self.virtual_subscribers.remove(key);
                }
            }
        } else {
            let _ = self.data_sub_tx.send(DataMsg::Unsubscribe {
                key: key.clone(),
                id: sub_id,
            });
        }
    }
}

impl PipelineModel {
    fn handle_msg(&mut self, msg: PipelineMsg) {
        match msg {
            PipelineMsg::CreatePlot { reply } => {
                let _ = reply.send(self.create_plot());
            }
            PipelineMsg::DestroyPlot { plot_id } => {
                self.destroy_plot(plot_id);
            }
            PipelineMsg::RegisterPlotChannel { plot_id, channel } => {
                self.register_plot_channel(plot_id, channel);
            }
            PipelineMsg::CreateStatisticsProvider {
                source_key,
                window_seconds,
                reply,
            } => {
                let _ = reply.send(self.create_statistics_provider(source_key, window_seconds));
            }
            PipelineMsg::RegisterStatisticsChannel {
                provider_id,
                channel,
            } => {
                self.register_statistics_channel(provider_id, channel);
            }
            PipelineMsg::CreateMathChannel {
                spec,
                input_rates,
                reply,
            } => {
                let _ = reply.send(self.create_math_channel(spec, input_rates));
            }
            PipelineMsg::DestroyMathChannel { output_key } => {
                self.destroy_virtual_stream(&output_key);
            }
            PipelineMsg::DestroyProcessor { id } => {
                self.destroy(id);
            }
            PipelineMsg::ResetPlot { plot_id, reply } => {
                let _ = reply.send(self.reset_plot(&plot_id));
            }
            PipelineMsg::ResetByPipelineId { id, reply } => {
                let _ = reply.send(self.reset_by_pipeline_id(id));
            }
            PipelineMsg::ResetByColumn { key, reply } => {
                let _ = reply.send(self.clear_column(&key));
            }
            PipelineMsg::ResetByStream { key, reply } => {
                let _ = reply.send(self.clear_stream(&key));
            }
            PipelineMsg::ResetByDevice { key, reply } => {
                let _ = reply.send(self.clear_device(&key));
            }
            PipelineMsg::ResetAllRuntimeState => {
                self.reset_all_runtime_state();
            }
            PipelineMsg::UpdatePlotColumns {
                plot_id,
                data_keys,
                max_sampling_rate,
                view_config,
                rates,
                backfills,
                reply,
            } => {
                let _ = reply.send(self.update_plot_columns(
                    plot_id,
                    data_keys,
                    max_sampling_rate,
                    view_config,
                    rates,
                    backfills,
                ));
            }
            PipelineMsg::UpdatePlotViewConfig {
                plot_id,
                view_config,
                backfills,
                reply,
            } => {
                let _ = reply.send(self.update_plot_view_config(plot_id, view_config, backfills));
            }
            PipelineMsg::SamplingRatesChanged { rates } => {
                self.handle_sampling_rates_changed(rates);
            }
            PipelineMsg::GetPlotDataKeys { plot_id, reply } => {
                let keys = self
                    .managed_plots
                    .get(&plot_id)
                    .map(|p| p.config.data_keys.clone());
                let _ = reply.send(keys);
            }
            PipelineMsg::Shutdown => unreachable!("handled in actor loop"),
        }
    }
}

impl PipelineModel {
    fn tick_plot_channels(&self) {
        for (plot_id, channel) in &self.plot_channels {
            if let Some(data) = self.get_merged_data_for_plot(plot_id) {
                if !data.is_empty() {
                    let _ = channel.send(data);
                }
            }
        }
    }

    fn tick_statistics_channels(&self) {
        for (provider_id, channel) in &self.statistics_channels {
            if let Some(provider) = self.stat_providers.get(provider_id) {
                let mut provider_locked = provider.lock().unwrap();
                let stats = provider_locked.get_output();
                let _ = channel.send(stats);
            }
        }
    }
}

impl PipelineModel {
    fn create_plot(&mut self) -> UiPlotState {
        let id = PlotId(Uuid::new_v4());
        let config = UiPlotState {
            plot_id: id,
            data_keys: vec![],
            max_sampling_rate: 0.0,
            active_view: PlotViewType::Timeseries,
            timeseries_config: TimeseriesConfig::default(),
            fft_config: FftConfig::default(),
        };
        self.managed_plots.insert(
            id,
            ManagedPlotPipeline {
                config: config.clone(),
                output_pipeline_ids: vec![],
                all_component_ids: vec![],
            },
        );
        config
    }

    fn destroy_plot(&mut self, plot_id: PlotId) {
        self.destroy_plot_components(&plot_id);
        self.plot_channels.remove(&plot_id);
    }

    fn apply_plot_config(
        &mut self,
        config: UiPlotState,
        rates: HashMap<UiColumnKey, f64>,
        backfills: HashMap<UiColumnKey, Vec<Point>>,
    ) -> Result<Vec<PipelineId>, String> {
        if self.managed_plots.contains_key(&config.plot_id) {
            self.destroy_plot_components(&config.plot_id);
        }

        // Cache provided rates
        for (key, rate) in &rates {
            self.cached_rates.insert(key.clone(), *rate);
        }

        let mut output_ids = Vec::new();
        let mut all_ids = Vec::new();
        for key in &config.data_keys {
            let rate = rates.get(key).copied().unwrap_or(0.0);
            let backfill = backfills.get(key).cloned().unwrap_or_default();
            match config.active_view_config() {
                ViewConfig::Timeseries(ts_config) => {
                    let id = self.create_timeseries_for_plot(
                        key,
                        &ts_config,
                        config.max_sampling_rate,
                        rate,
                        backfill,
                    );
                    output_ids.push(id);
                    all_ids.push(id);
                }
                ViewConfig::Fft(fft_config) => {
                    let (fft_id, src_id) =
                        self.create_fft_chain_for_plot(key, &fft_config, rate, backfill)?;
                    output_ids.push(fft_id);
                    all_ids.push(src_id);
                    all_ids.push(fft_id);
                }
            }
        }
        self.managed_plots.insert(
            config.plot_id,
            ManagedPlotPipeline {
                config,
                output_pipeline_ids: output_ids.clone(),
                all_component_ids: all_ids,
            },
        );
        Ok(output_ids)
    }

    fn register_plot_channel(&mut self, plot_id: PlotId, channel: Channel<PlotData>) {
        self.plot_channels.insert(plot_id, channel);
    }

    fn get_merged_data_for_plot(&self, plot_id: &PlotId) -> Option<PlotData> {
        let managed_plot = self.managed_plots.get(plot_id)?;
        let mut data_to_merge = Vec::with_capacity(managed_plot.output_pipeline_ids.len());
        for id in &managed_plot.output_pipeline_ids {
            if let Some(p) = self.pipelines.get(id) {
                let pipeline = p.lock().unwrap();
                data_to_merge.push(pipeline.get_output());
            }
        }
        if data_to_merge.is_empty() {
            None
        } else {
            Some(k_way_merge_plot_data(data_to_merge))
        }
    }

    fn destroy_plot_components(&mut self, plot_id: &PlotId) {
        if let Some(plot) = self.managed_plots.remove(plot_id) {
            for id in plot.all_component_ids.iter().rev() {
                self.destroy(*id);
            }
        }
    }

    fn update_plot_columns(
        &mut self,
        plot_id: PlotId,
        data_keys: Vec<UiColumnKey>,
        max_sampling_rate: f64,
        view_config: ViewConfig,
        rates: HashMap<UiColumnKey, f64>,
        backfills: HashMap<UiColumnKey, Vec<Point>>,
    ) -> Result<UiPlotState, String> {
        let mut config = self
            .managed_plots
            .get(&plot_id)
            .map(|managed| managed.config.clone())
            .unwrap_or(UiPlotState {
                plot_id,
                data_keys: vec![],
                max_sampling_rate: 0.0,
                active_view: PlotViewType::Timeseries,
                timeseries_config: TimeseriesConfig::default(),
                fft_config: FftConfig::default(),
            });

        config.data_keys = data_keys;
        config.max_sampling_rate = max_sampling_rate;
        match view_config {
            ViewConfig::Timeseries(timeseries_config) => {
                config.active_view = PlotViewType::Timeseries;
                config.timeseries_config = timeseries_config;
            }
            ViewConfig::Fft(fft_config) => {
                config.active_view = PlotViewType::Fft;
                config.fft_config = fft_config;
            }
        }

        self.apply_plot_config(config, rates, backfills)?;
        self.managed_plots
            .get(&plot_id)
            .map(|managed| managed.config.clone())
            .ok_or_else(|| "Plot not found after update.".to_string())
    }

    fn update_plot_view_config(
        &mut self,
        plot_id: PlotId,
        view_config: ViewConfig,
        backfills: HashMap<UiColumnKey, Vec<Point>>,
    ) -> Result<UiPlotState, String> {
        let managed = self.managed_plots.get(&plot_id).ok_or("Plot not found")?;
        let mut config = managed.config.clone();

        match view_config {
            ViewConfig::Timeseries(timeseries_config) => {
                config.active_view = PlotViewType::Timeseries;
                config.timeseries_config = timeseries_config;
            }
            ViewConfig::Fft(fft_config) => {
                config.active_view = PlotViewType::Fft;
                config.fft_config = fft_config;
            }
        }

        let data_keys = config.data_keys.clone();

        let mut rates = HashMap::new();
        for key in &data_keys {
            if let Some(&rate) = self.cached_rates.get(key) {
                rates.insert(key.clone(), rate);
            }
        }

        config.max_sampling_rate = rates.values().copied().fold(0.0_f64, f64::max);

        self.apply_plot_config(config, rates, backfills)?;
        self.managed_plots
            .get(&plot_id)
            .map(|managed| managed.config.clone())
            .ok_or_else(|| "Plot not found after update.".to_string())
    }

    fn handle_sampling_rates_changed(&mut self, new_rates: HashMap<UiColumnKey, f64>) {
        // Update cached_rates and collect keys that actually changed
        let mut changed_keys = Vec::new();
        for (key, rate) in &new_rates {
            let old = self.cached_rates.get(key).copied();
            if old != Some(*rate) {
                self.cached_rates.insert(key.clone(), *rate);
                changed_keys.push(key.clone());
            }
        }

        if changed_keys.is_empty() {
            return;
        }

        // Find managed plots that reference any changed key
        let affected_plot_ids: Vec<PlotId> = self
            .managed_plots
            .iter()
            .filter(|(_, managed)| {
                managed
                    .config
                    .data_keys
                    .iter()
                    .any(|k| changed_keys.contains(k))
            })
            .map(|(id, _)| *id)
            .collect();

        for plot_id in affected_plot_ids {
            let managed = self.managed_plots.get(&plot_id).unwrap();
            let mut config = managed.config.clone();

            // Rebuild rates from cache
            let mut rates = HashMap::new();
            for k in &config.data_keys {
                if let Some(&r) = self.cached_rates.get(k) {
                    rates.insert(k.clone(), r);
                }
            }
            config.max_sampling_rate = rates.values().copied().fold(0.0_f64, f64::max);

            // Empty backfills — device restarted streams, old data is stale
            let backfills = HashMap::new();
            let _ = self.apply_plot_config(config, rates, backfills);
        }
    }
}

impl PipelineModel {
    fn new_virtual_key() -> UiColumnKey {
        let stream_id = NEXT_MATH_STREAM_ID.fetch_add(1, Ordering::Relaxed);
        UiColumnKey::new(VIRTUAL_ROUTE.clone(), stream_id, 0)
    }

    fn create_timeseries_for_plot(
        &mut self,
        key: &UiColumnKey,
        config: &TimeseriesConfig,
        max_sr: f64,
        rate: f64,
        backfill: Vec<Point>,
    ) -> PipelineId {
        match config.decimation_method {
            DecimationMethod::Fpcs => {
                let ratio = ((max_sr * config.window_seconds)
                    / (10.0 * config.resolution_multiplier as f64))
                    .round() as usize;
                let pipeline =
                    StreamingFpcsPipeline::new(key.clone(), ratio.max(1), config.window_seconds);
                self.spawn_root_pipeline(pipeline, key.clone(), rate, backfill)
            }
            DecimationMethod::None => {
                let pipeline = PassthroughPipeline::new(key.clone(), config.window_seconds);
                self.spawn_root_pipeline(pipeline, key.clone(), rate, backfill)
            }
        }
    }

    fn create_fft_chain_for_plot(
        &mut self,
        key: &UiColumnKey,
        config: &FftConfig,
        rate: f64,
        backfill: Vec<Point>,
    ) -> Result<(PipelineId, PipelineId), String> {
        let detrend_pipeline = DetrendPipeline::new(
            key.clone(),
            config.window_seconds,
            config.detrend_method.clone(),
        );
        let detrend_id = self.spawn_root_pipeline(detrend_pipeline, key.clone(), rate, backfill);

        let fft_pipeline = FftPipeline::new();
        let (fft_id, fft_input_tx) = self.spawn_derived_pipeline(fft_pipeline, key.clone());

        let handle = self
            .pipeline_threads
            .get(&detrend_id)
            .ok_or("Detrend handle not found")?;

        handle
            .cmd_tx
            .send(PipelineCommand::AddSubscriber(fft_input_tx))
            .map_err(|e| e.to_string())?;

        Ok((fft_id, detrend_id))
    }
}

impl PipelineModel {
    fn spawn_root_pipeline<P>(
        &mut self,
        pipeline: P,
        source_key: UiColumnKey,
        rate: f64,
        backfill: Vec<Point>,
    ) -> PipelineId
    where
        P: Pipeline + 'static,
    {
        let id = pipeline.id();
        let pipeline_arc = Arc::new(Mutex::new(pipeline));
        self.pipelines.insert(id, pipeline_arc.clone());
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        let (data_tx, data_rx) = bounded(128);
        let (cmd_tx, cmd_rx) = bounded(16);

        self.subscribe_mut(&source_key, sub_id, data_tx);

        let _ = cmd_tx.send(PipelineCommand::Hydrate {
            sampling_rate: Some(rate),
            backfill,
        });

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
                            pipeline_arc.lock().unwrap().process_command(cmd);
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
                thread_type: ThreadType::Root { sub_id, source_key },
            },
        );
        id
    }

    fn spawn_derived_pipeline<P>(
        &mut self,
        pipeline: P,
        source_key: UiColumnKey,
    ) -> (PipelineId, Sender<(PlotData, f64)>)
    where
        P: Pipeline + 'static,
    {
        let id = pipeline.id();
        let pipeline_arc = Arc::new(Mutex::new(pipeline));
        self.pipelines.insert(id, pipeline_arc.clone());

        let (data_tx, data_rx) = bounded(128);
        let (cmd_tx, cmd_rx) = bounded(16);
        let handle = thread::Builder::new()
            .name(format!("pipeline-derived-{:?}", id))
            .spawn(move || loop {
                select! {
                    recv(data_rx) -> msg => match msg {
                        Ok(batch) => pipeline_arc.lock().unwrap().process_derived_batch(batch),
                        Err(_) => break,
                    },
                    recv(cmd_rx) -> msg => match msg {
                        Ok(cmd) => {
                            if matches!(cmd, PipelineCommand::Shutdown) { break; }
                            pipeline_arc.lock().unwrap().process_command(cmd);
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
                thread_type: ThreadType::Derived { source_key },
            },
        );
        (id, data_tx)
    }

    fn spawn_producer_mono<P>(
        &mut self,
        pipeline: P,
        source_key: UiColumnKey,
        output_key: UiColumnKey,
    ) -> PipelineId
    where
        P: Pipeline + 'static,
    {
        let id = pipeline.id();
        let pipeline_arc = Arc::new(Mutex::new(pipeline));
        self.pipelines.insert(id, pipeline_arc.clone());

        let sub_id = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        let (data_tx, data_rx) = bounded(128);
        let (cmd_tx, cmd_rx) = bounded(16);

        self.subscribe_mut(&source_key, sub_id, data_tx);

        let handle = thread::Builder::new()
            .name(format!("pipeline-producer-mono-{:?}", id))
            .spawn(move || loop {
                select! {
                    recv(data_rx) -> msg => match msg {
                        Ok(batch) => pipeline_arc.lock().unwrap().process_batch(batch),
                        Err(_) => break,
                    },
                    recv(cmd_rx) -> msg => match msg {
                        Ok(cmd) => {
                            if matches!(cmd, PipelineCommand::Shutdown) { break; }
                            pipeline_arc.lock().unwrap().process_command(cmd);
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
                thread_type: ThreadType::ProducerMono {
                    sub_id,
                    source_key,
                    output_key,
                },
            },
        );
        id
    }

    fn spawn_producer_bi<P>(
        &mut self,
        pipeline: P,
        source_a: UiColumnKey,
        source_b: UiColumnKey,
        output_key: UiColumnKey,
    ) -> PipelineId
    where
        P: Pipeline + 'static,
    {
        let id = pipeline.id();
        let pipeline_arc = Arc::new(Mutex::new(pipeline));
        self.pipelines.insert(id, pipeline_arc.clone());

        let sub_id_a = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        let sub_id_b = self.next_sub_id.fetch_add(1, Ordering::Relaxed);

        let (a_tx, a_rx) = bounded(128);
        let (b_tx, b_rx) = bounded(128);
        let (cmd_tx, cmd_rx) = bounded(16);

        self.subscribe_mut(&source_a, sub_id_a, a_tx);
        self.subscribe_mut(&source_b, sub_id_b, b_tx);

        let handle = thread::Builder::new()
            .name(format!("pipeline-producer-bi-{:?}", id))
            .spawn(move || loop {
                select! {
                    recv(a_rx) -> msg => match msg {
                        Ok(batch) => pipeline_arc.lock().unwrap().process_batch(batch),
                        Err(_) => break,
                    },
                    recv(b_rx) -> msg => match msg {
                        Ok(batch) => pipeline_arc.lock().unwrap().process_batch(batch),
                        Err(_) => break,
                    },
                    recv(cmd_rx) -> msg => match msg {
                        Ok(cmd) => {
                            if matches!(cmd, PipelineCommand::Shutdown) { break; }
                            pipeline_arc.lock().unwrap().process_command(cmd);
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
                thread_type: ThreadType::ProducerBi {
                    sub_id_a,
                    sub_id_b,
                    source_a,
                    source_b,
                    output_key,
                },
            },
        );
        id
    }
}

impl PipelineModel {
    fn create_statistics_provider(
        &mut self,
        source_key: UiColumnKey,
        window_seconds: f64,
    ) -> PipelineId {
        let provider = StreamingStatisticsProvider::new(source_key.clone(), window_seconds);
        let id = provider.id();
        let provider_arc = Arc::new(Mutex::new(provider));
        self.stat_providers.insert(id, provider_arc.clone());

        let (cmd_tx, cmd_rx) = bounded(16);
        let (data_tx, data_rx) = bounded(128);

        let sub_id = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        self.subscribe_mut(&source_key, sub_id, data_tx);

        let handle = thread::Builder::new()
            .name(format!("pipeline-stats-{:?}", id))
            .spawn(move || loop {
                select! {
                    recv(data_rx) -> msg => match msg {
                        Ok(batch) => provider_arc.lock().unwrap().process_batch(batch),
                        Err(_) => break,
                    },
                    recv(cmd_rx) -> msg => match msg {
                        Ok(PipelineCommand::Shutdown) => break,
                        Ok(PipelineCommand::ResetSelf) => {
                            provider_arc.lock().unwrap().reset();
                        }
                        Ok(_) => {},
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
                thread_type: ThreadType::Statistics { sub_id, source_key },
            },
        );

        id
    }

    fn register_statistics_channel(
        &mut self,
        provider_id: PipelineId,
        channel: Channel<ColumnStatistics>,
    ) {
        self.statistics_channels.insert(provider_id, channel);
    }
}

impl PipelineModel {
    fn create_math_channel(
        &mut self,
        spec: MathChannelConfig,
        input_rates: HashMap<UiColumnKey, Option<f64>>,
    ) -> Result<MathChannelResult, String> {
        let out_key = Self::new_virtual_key();
        let virtual_tx = self.virtual_data_tx.clone();

        let (pid, out_rate) = match spec {
            MathChannelConfig::Arithmetic { a, op, b } => {
                let rate_a = input_rates.get(&a).copied().flatten();
                let rate_b = input_rates.get(&b).copied().flatten();
                let out_rate = match (rate_a, rate_b) {
                    (Some(ra), Some(rb)) => Some(ra.max(rb)),
                    (Some(r), None) | (None, Some(r)) => Some(r),
                    _ => None,
                };

                let p =
                    ArithmeticProducer::new(a.clone(), b.clone(), op, out_key.clone(), virtual_tx);
                let pid = self.spawn_producer_bi(p, a, b, out_key.clone());

                if let Some(h) = self.pipeline_threads.get(&pid) {
                    let _ = h.cmd_tx.send(PipelineCommand::Hydrate {
                        sampling_rate: out_rate,
                        backfill: vec![],
                    });
                }
                (pid, out_rate)
            }
            MathChannelConfig::Biquad { src, kind, params } => {
                let out_rate = input_rates.get(&src).copied().flatten();

                let p = BiquadProducer::new(src.clone(), kind, params, out_key.clone(), virtual_tx);
                let pid = self.spawn_producer_mono(p, src, out_key.clone());

                if let Some(h) = self.pipeline_threads.get(&pid) {
                    let _ = h.cmd_tx.send(PipelineCommand::Hydrate {
                        sampling_rate: out_rate,
                        backfill: vec![],
                    });
                }
                (pid, out_rate)
            }
        };

        // Cache the output rate
        if let Some(rate) = out_rate {
            self.cached_rates.insert(out_key.clone(), rate);
        }

        self.virtual_streams.insert(out_key.clone(), pid);
        Ok(MathChannelResult {
            pipeline_id: pid,
            output_key: out_key,
            output_rate: out_rate,
        })
    }

    fn destroy_virtual_stream(&mut self, key: &UiColumnKey) {
        if let Some(pid) = self.virtual_streams.remove(key) {
            self.destroy(pid);
        }
        self.virtual_subscribers.remove(key);
    }
}

impl PipelineModel {
    fn pipeline_ids_matching(
        &self,
        mut predicate: impl FnMut(&ThreadType) -> bool,
    ) -> Vec<PipelineId> {
        self.pipeline_threads
            .iter()
            .filter_map(|(id, handle)| predicate(&handle.thread_type).then_some(*id))
            .collect()
    }

    fn send_reset_self(&self, ids: &[PipelineId]) {
        for id in ids {
            if let Some(handle) = self.pipeline_threads.get(id) {
                let _ = handle.cmd_tx.send(PipelineCommand::ResetSelf);
            }
        }
    }

    fn destroy(&mut self, id: PipelineId) {
        if let Some(handle) = self.pipeline_threads.remove(&id) {
            match &handle.thread_type {
                ThreadType::Statistics { sub_id, source_key } => {
                    self.unsubscribe(source_key, *sub_id);
                }
                ThreadType::Root { sub_id, source_key } => {
                    self.unsubscribe(source_key, *sub_id);
                }
                ThreadType::Derived { .. } => {}
                ThreadType::ProducerMono {
                    sub_id,
                    source_key,
                    output_key: _,
                } => {
                    self.unsubscribe(source_key, *sub_id);
                }
                ThreadType::ProducerBi {
                    sub_id_a,
                    sub_id_b,
                    source_a,
                    source_b,
                    output_key: _,
                } => {
                    self.unsubscribe(source_a, *sub_id_a);
                    self.unsubscribe(source_b, *sub_id_b);
                }
            }

            let _ = handle.cmd_tx.send(PipelineCommand::Shutdown);
            if let Err(e) = handle.handle.join() {
                eprintln!("[Pipeline] Join failed for {:?}: {:?}", id, e);
            }
        }

        let _ = self.pipelines.remove(&id);
        let _ = self.stat_providers.remove(&id);
        let _ = self.statistics_channels.remove(&id);
    }

    fn reset_plot(&mut self, plot_id: &PlotId) -> Result<(), String> {
        let managed = self.managed_plots.get(plot_id).ok_or("Plot not found")?;
        let ids: Vec<PipelineId> = managed.all_component_ids.clone();
        for id in ids {
            self.reset_by_pipeline_id(id)?;
        }
        Ok(())
    }

    fn reset_by_pipeline_id(&mut self, id: PipelineId) -> Result<(), String> {
        let handle = self.pipeline_threads.get(&id).ok_or("Pipeline not found")?;
        let cmd_tx = handle.cmd_tx.clone();

        let hydrate_cmd = match &handle.thread_type {
            ThreadType::Root { source_key, .. } | ThreadType::Statistics { source_key, .. } => {
                let rate = self.cached_rates.get(source_key).copied();
                Some(PipelineCommand::Hydrate {
                    sampling_rate: rate,
                    backfill: vec![],
                })
            }
            ThreadType::ProducerMono { source_key, .. } => {
                let rate = self.cached_rates.get(source_key).copied();
                Some(PipelineCommand::Hydrate {
                    sampling_rate: rate,
                    backfill: vec![],
                })
            }
            ThreadType::ProducerBi {
                source_a, source_b, ..
            } => {
                let rate_a = self.cached_rates.get(source_a).copied();
                let rate_b = self.cached_rates.get(source_b).copied();
                let rate = match (rate_a, rate_b) {
                    (Some(ra), Some(rb)) => Some(ra.max(rb)),
                    (Some(r), None) | (None, Some(r)) => Some(r),
                    _ => None,
                };
                Some(PipelineCommand::Hydrate {
                    sampling_rate: rate,
                    backfill: vec![],
                })
            }
            ThreadType::Derived { .. } => None,
        };

        cmd_tx
            .send(PipelineCommand::ResetSelf)
            .map_err(|e| e.to_string())?;
        if let Some(hydrate) = hydrate_cmd {
            let _ = cmd_tx.send(hydrate);
        }

        Ok(())
    }

    fn clear_column(&mut self, column_key: &UiColumnKey) -> Result<(), String> {
        let ids = self.pipeline_ids_matching(|thread_type| match thread_type {
            ThreadType::Root { source_key, .. }
            | ThreadType::Statistics { source_key, .. }
            | ThreadType::Derived { source_key }
            | ThreadType::ProducerMono { source_key, .. } => source_key == column_key,
            ThreadType::ProducerBi {
                source_a, source_b, ..
            } => source_a == column_key || source_b == column_key,
        });
        self.send_reset_self(&ids);

        for (plot_id, managed) in &self.managed_plots {
            if managed.config.data_keys.iter().any(|k| k == column_key) {
                if let Some(ch) = self.plot_channels.get(plot_id) {
                    let _ = ch.send(PlotData::empty());
                }
            }
        }
        Ok(())
    }

    fn clear_stream(&mut self, any_key_in_stream: &UiColumnKey) -> Result<(), String> {
        let stream_key = any_key_in_stream.stream_key();
        let ids = self.pipeline_ids_matching(|thread_type| match thread_type {
            ThreadType::Root { source_key, .. }
            | ThreadType::Statistics { source_key, .. }
            | ThreadType::Derived { source_key }
            | ThreadType::ProducerMono { source_key, .. } => source_key.stream_key() == stream_key,
            ThreadType::ProducerBi {
                source_a, source_b, ..
            } => source_a.stream_key() == stream_key || source_b.stream_key() == stream_key,
        });
        self.send_reset_self(&ids);

        Ok(())
    }

    fn clear_device(&mut self, any_key_on_device: &UiColumnKey) -> Result<(), String> {
        let device_key = any_key_on_device.device_key();
        let ids = self.pipeline_ids_matching(|thread_type| match thread_type {
            ThreadType::Root { source_key, .. }
            | ThreadType::Statistics { source_key, .. }
            | ThreadType::Derived { source_key }
            | ThreadType::ProducerMono { source_key, .. } => source_key.device_key() == device_key,
            ThreadType::ProducerBi {
                source_a, source_b, ..
            } => source_a.device_key() == device_key || source_b.device_key() == device_key,
        });
        self.send_reset_self(&ids);

        Ok(())
    }

    fn reset_all_runtime_state(&mut self) {
        for handle in self.pipeline_threads.values() {
            let _ = handle.cmd_tx.send(PipelineCommand::ResetSelf);
        }

        for channel in self.plot_channels.values() {
            let _ = channel.send(PlotData::empty());
        }
    }
}
