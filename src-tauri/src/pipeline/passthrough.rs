use super::{Pipeline, PipelineCommand};
use crate::shared::{PipelineId, PlotData, Point, UiColumnKey};
use crate::state::data_manager::{BatchedData, SessionId};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct PassthroughPipeline {
    id: PipelineId,
    source_key: UiColumnKey,
    window_seconds: f64,
    buffer: Arc<Mutex<VecDeque<Point>>>,
    capacity: usize,
    last_session_id: Option<SessionId>,
}

impl PassthroughPipeline {
    pub fn new(source_key: UiColumnKey, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            capacity: 0,
            last_session_id: None,
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
            ys.push(if p.y.is_nan() { None } else { Some(p.y) });
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

        // Session boundary: insert gap marker, keep old data
        if let Some(last_sid) = self.last_session_id {
            if last_sid != batch.session_id {
                if let Some(first_point) = batch.points.first() {
                    if self.capacity > 0 && buffer.len() >= self.capacity {
                        buffer.pop_front();
                    }
                    buffer.push_back(Point::new(first_point.x, f64::NAN));
                }
            }
        }
        self.last_session_id = Some(batch.session_id);

        for p in batch.points.iter() {
            if self.capacity > 0 && buffer.len() >= self.capacity {
                buffer.pop_front();
            }
            buffer.push_back(*p);
        }
    }

    fn process_command(&mut self, cmd: PipelineCommand) {
        match cmd {
            PipelineCommand::ResetSelf => {
                self.buffer.lock().unwrap().clear();
                self.last_session_id = None;
            }
            PipelineCommand::Hydrate {
                sampling_rate,
                backfill,
            } => {
                if let Some(sr) = sampling_rate {
                    self.capacity = ((sr * self.window_seconds).ceil() as usize).max(2);
                    self.buffer.lock().unwrap().reserve(self.capacity);
                }
                if !backfill.is_empty() {
                    let mut buffer = self.buffer.lock().unwrap();
                    buffer.clear();
                    buffer.extend(backfill);
                }
            }
            _ => {}
        }
    }
}
