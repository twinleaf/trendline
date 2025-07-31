use super::decimation::StreamingFpcsPipeline;
use super::detrend::DetrendPipeline;
use super::fft::FftPipeline;
use super::passthrough::PassthroughPipeline;
use super::Pipeline;
use crate::pipeline::statistics::StreamingStatisticsProvider;
use crate::pipeline::StatisticsProvider;
use crate::shared::{DataColumnId, DetrendMethod, PipelineId, StreamStatistics};
use crate::state::capture::CaptureState;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct ProcessingManager {
    pub pipelines: HashMap<PipelineId, Arc<Mutex<dyn Pipeline>>>,
    pub stat_providers: HashMap<PipelineId, Arc<Mutex<dyn StatisticsProvider>>>,
}

impl ProcessingManager {

    pub fn new_with_ticker(capture_state: CaptureState) -> Arc<Mutex<Self>> {
        let manager = Arc::new(Mutex::new(Self {
            pipelines: HashMap::new(),
            stat_providers: HashMap::new(),
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

                    thread::sleep(Duration::from_millis(100));
                }
            })
            .expect("Failed to spawn pipeline ticker thread.");

        manager
    }

    pub fn create_fpcs_pipeline(&mut self, source_key: DataColumnId, ratio: usize, window_seconds: f64) -> PipelineId {
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
    pub fn get_statistics_data(&self, id: PipelineId, capture_state: &CaptureState) -> Option<StreamStatistics> {
        if let Some(provider_mutex) = self.stat_providers.get(&id) {
            let mut provider = provider_mutex.lock().unwrap();
            provider.update(capture_state);
            Some(provider.get_output())
        } else 
        { None }
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
    }
}