use super::{Pipeline, PipelineCommand};
use crate::shared::{DataColumnId, PipelineId, PlotData, Point};
use crate::state::capture::{BatchedData, CaptureState};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct PassthroughPipeline {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,
    buffer: Arc<Mutex<VecDeque<Point>>>,
    capacity: usize,
    sample_rate: Option<f64>,
}

impl PassthroughPipeline {
    pub fn new(source_key: DataColumnId, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            capacity: 0,
            sample_rate: None,
        }
    }
}

impl Pipeline for PassthroughPipeline {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData {
        let buffer = self.buffer.lock().unwrap();
        if buffer.is_empty() {
            return PlotData::empty();
        }
        let mut ts = Vec::with_capacity(buffer.len());
        let mut ys = Vec::with_capacity(buffer.len());
        for p in buffer.iter() {
            ts.push(p.x);
            ys.push(p.y);
        }
        PlotData {
            timestamps: ts,
            series_data: vec![ys],
        }
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        if batch.key != self.source_key {
            return;
        }
        let mut buffer = self.buffer.lock().unwrap();
        for p in batch.points.iter() {
            if self.capacity > 0 && buffer.len() >= self.capacity {
                buffer.pop_front();
            }
            buffer.push_back(*p);
        }
    }

    fn process_command(&mut self, cmd: PipelineCommand, capture_state: &CaptureState) {
        if let PipelineCommand::Hydrate = cmd {
            if let Some(sr) = capture_state.get_effective_sampling_rate(&self.source_key) {
                self.sample_rate = Some(sr);
                self.capacity = ((sr * self.window_seconds).ceil() as usize).max(2);
                self.buffer.lock().unwrap().reserve(self.capacity);
            }
            let Some(latest_time) =
                capture_state.get_latest_unified_timestamp(&[self.source_key.clone()])
            else {
                return;
            };
            let start_time = latest_time - self.window_seconds;
            let raw_data_vecs = capture_state.get_data_across_sessions_for_keys(
                &[self.source_key.clone()],
                start_time,
                latest_time,
            );
            if let Some(points) = raw_data_vecs.get(0) {
                let mut buffer = self.buffer.lock().unwrap();
                buffer.clear();
                buffer.extend(points);
            }
        }
    }
}
