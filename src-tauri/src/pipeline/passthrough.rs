use super::Pipeline;
use crate::shared::{DataColumnId, PipelineId, PlotData};
use crate::state::capture::CaptureState;
use crate::pipeline::buffer::DoubleBuffer;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct PassthroughPipeline {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,

    output: Arc<Mutex<DoubleBuffer<PlotData>>>,
}

impl PassthroughPipeline {
    pub fn new(source_key: DataColumnId, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            output: Arc::new(Mutex::new(DoubleBuffer::new())),
        }
    }
}

impl Pipeline for PassthroughPipeline {
    fn id(&self) -> PipelineId { self.id }

    fn get_output(&self) -> PlotData {
        self.output.lock().unwrap().read_with(|data| data.clone())
    }

    fn update(&mut self, capture_state: &CaptureState) {
        let Some(latest_time) = capture_state.get_latest_unified_timestamp(&[self.source_key.clone()]) else {
            return;
        };
        let min_time = latest_time - self.window_seconds;
        let raw_data_vecs = capture_state.get_data_across_sessions_for_keys(
            &[self.source_key.clone()],
            min_time,
            latest_time,
        );
        
        self.output.lock().unwrap().write_with(|plot_data_back_buffer| {
            if let Some(points) = raw_data_vecs.get(0) {
                plot_data_back_buffer.timestamps = points.iter().map(|p| p.x).collect();
                plot_data_back_buffer.series_data = vec![points.iter().map(|p| p.y).collect()];
            } else {
                *plot_data_back_buffer = PlotData::empty();
            }
        });
    }

    fn get_source_sampling_rate(&self, capture_state: &CaptureState) -> Option<f64> {
        capture_state.get_effective_sampling_rate(&self.source_key)
    }
}