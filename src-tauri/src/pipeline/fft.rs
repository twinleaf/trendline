use super::Pipeline;
use crate::pipeline::buffer::DoubleBuffer;
use crate::shared::{PipelineId, PlotData};
use crate::state::capture::CaptureState;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use welch_sde::{Build, SpectralDensity};

pub struct FftPipeline {
    id: PipelineId,
    source_pipeline: Arc<Mutex<dyn Pipeline>>,
    output: Arc<Mutex<DoubleBuffer<PlotData>>>,
}

impl FftPipeline {
    pub fn new(source_pipeline: Arc<Mutex<dyn Pipeline>>) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_pipeline,
            output: Arc::new(Mutex::new(DoubleBuffer::new())),
        }
    }
}

impl Pipeline for FftPipeline {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData { 
        self.output.lock().unwrap().read_with(|data| data.clone())
    }

    fn update(&mut self, capture_state: &CaptureState) { 
        let source_output = self.source_pipeline.lock().unwrap().get_output();

        let y_values = if let Some(data) = source_output.series_data.get(0) {
            data
        } else {
            return;
        };
        if y_values.len() < 16 {
            self.output.lock().unwrap().write_with(|b| *b = PlotData::empty());
            return;
        }

        let Some(sampling_rate) = self.get_source_sampling_rate(capture_state) else {
            self.output.lock().unwrap().write_with(|b| *b = PlotData::empty());
            return;
        };

        let welch: SpectralDensity<f64> = SpectralDensity::builder(y_values, sampling_rate).build();
        let psd = welch.periodogram();
        let asd: Vec<f64> = psd.to_vec().iter().map(|&p| p.sqrt()).collect();
        let frequencies = psd.frequency().to_vec();

        self.output
            .lock()
            .unwrap()
            .write_with(|plot_data_back_buffer| {
                plot_data_back_buffer.timestamps = frequencies;
                plot_data_back_buffer.series_data = vec![asd];
            });
    }

    fn get_source_sampling_rate(&self, capture_state: &CaptureState) -> Option<f64> {
        self.source_pipeline
            .lock()
            .unwrap()
            .get_source_sampling_rate(capture_state)
    }
}