use super::{Pipeline, PipelineCommand};
use crate::shared::{DataColumnId, PipelineId, PlotData, Point};
use crate::state::capture::CaptureState;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Debug)]
enum FpcsLastRetained {
    None,
    Max,
    Min,
}

pub struct StreamingFpcsPipeline {
    id: PipelineId,
    source_key: DataColumnId,
    last_processed_time: f64,
    ratio: usize,
    window_seconds: f64,
    output: Arc<Mutex<VecDeque<Point>>>,
    capacity: usize,

    // Internal state for the FPCS algorithm
    counter: usize,
    potential_point: Option<Point>,
    last_retained_flag: FpcsLastRetained,
    window_max_point: Option<Point>,
    window_min_point: Option<Point>,
}

impl StreamingFpcsPipeline {
    pub fn new(source_key: DataColumnId, ratio: usize, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            last_processed_time: 0.0,
            ratio,
            window_seconds,
            output: Arc::new(Mutex::new(VecDeque::new())),
            capacity: 0,
            counter: 0,
            potential_point: None,
            last_retained_flag: FpcsLastRetained::None,
            window_max_point: None,
            window_min_point: None,
        }
    }

    /// The core stateful algorithm, now private to the pipeline.
    fn process_point(&mut self, p: Point) {
        if self.window_max_point.is_none() {
            self.retain_point(p);
            self.window_max_point = Some(p);
            self.window_min_point = Some(p);
            self.counter = 1;
            return;
        }

        let mut max_p = self.window_max_point.unwrap();
        let mut min_p = self.window_min_point.unwrap();

        self.counter += 1;

        if p.y >= max_p.y {
            max_p = p;
        } else if p.y < min_p.y {
            min_p = p;
        }

        if self.counter >= self.ratio {
            if min_p.x < max_p.x {
                if self.last_retained_flag == FpcsLastRetained::Min
                    && self.potential_point != Some(min_p)
                {
                    if let Some(pp) = self.potential_point {
                        self.retain_point(pp);
                    }
                }
                self.retain_point(min_p);
                self.potential_point = Some(max_p);
                min_p = max_p;
                self.last_retained_flag = FpcsLastRetained::Min;
            } else {
                if self.last_retained_flag == FpcsLastRetained::Max
                    && self.potential_point != Some(max_p)
                {
                    if let Some(pp) = self.potential_point {
                        self.retain_point(pp);
                    }
                }
                self.retain_point(max_p);
                self.potential_point = Some(min_p);
                max_p = min_p;
                self.last_retained_flag = FpcsLastRetained::Max;
            }
            self.counter = 0;
        }
        self.window_max_point = Some(max_p);
        self.window_min_point = Some(min_p);
    }

    /// Retains a point in the output buffer, managing capacity.
    fn retain_point(&mut self, p: Point) {
        let mut output = self.output.lock().unwrap();
        if self.capacity > 0 && output.len() >= self.capacity {
            output.pop_front();
        }
        output.push_back(p);
    }
}

impl Pipeline for StreamingFpcsPipeline {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData {
        let output = self.output.lock().unwrap();
        if output.is_empty() {
            return PlotData::empty();
        }

        let mut timestamps = Vec::with_capacity(output.len());
        let mut series_data = Vec::with_capacity(output.len());

        for point in output.iter() {
            timestamps.push(point.x);
            series_data.push(point.y);
        }

        PlotData {
            timestamps,
            series_data: vec![series_data],
        }
    }

    fn process_batch(&mut self, batch: Arc<crate::state::capture::BatchedData>) {
        // Ignore batches that have already been processed during hydration.
        if batch.t_max <= self.last_processed_time {
            return;
        }
        for point in batch.points.iter() {
            self.process_point(*point);
        }
        self.last_processed_time = batch.t_max;
    }

    fn process_command(&mut self, cmd: PipelineCommand, capture_state: &CaptureState) {
        match cmd {
            PipelineCommand::Hydrate => {
                println!("[FPCS Pipeline {:?}] Received Hydrate command.", self.id);
                if self.capacity == 0 && self.window_seconds > 0.0 {
                    if let Some(sampling_rate) =
                        capture_state.get_effective_sampling_rate(&self.source_key)
                    {
                        if sampling_rate > 0.0 {
                            let output_rate_approx = (2.0 * sampling_rate) / self.ratio as f64;
                            let new_capacity =
                                (output_rate_approx * self.window_seconds).ceil() as usize;
                            self.capacity = new_capacity.max(2);
                            println!(
                                "[FPCS Pipeline {:?}] Hydrated with capacity {}",
                                self.id, self.capacity
                            );
                        }
                    }
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
                    if points.is_empty() {
                        return;
                    }
                    println!(
                        "[FPCS Pipeline {:?}] Backfilling with {} points.",
                        self.id,
                        points.len()
                    );
                    for point in points {
                        self.process_point(*point);
                    }
                    self.last_processed_time = latest_time;
                }
            }
            _ => {}
        }
    }
}