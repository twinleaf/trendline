use super::{Pipeline, PipelineCommand};
use crate::shared::{PipelineId, PlotData, Point, UiColumnKey};
use crate::state::data_manager::{BatchedData, SessionId};
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
    source_key: UiColumnKey,
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
    last_session_id: Option<SessionId>,
    last_sample_number: Option<u32>,
}

impl StreamingFpcsPipeline {
    pub fn new(source_key: UiColumnKey, ratio: usize, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            last_processed_time: f64::NEG_INFINITY,
            ratio,
            window_seconds,
            output: Arc::new(Mutex::new(VecDeque::new())),
            capacity: 0,
            counter: 0,
            potential_point: None,
            last_retained_flag: FpcsLastRetained::None,
            window_max_point: None,
            window_min_point: None,
            last_session_id: None,
            last_sample_number: None,
        }
    }

    #[inline]
    fn reset_internal(&mut self) {
        self.output.lock().unwrap().clear();
        self.reset_algorithm_state();
    }

    #[inline]
    fn reset_algorithm_state(&mut self) {
        self.counter = 0;
        self.potential_point = None;
        self.last_retained_flag = FpcsLastRetained::None;
        self.window_max_point = None;
        self.window_min_point = None;
        self.last_processed_time = f64::NEG_INFINITY;
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
            series_data.push(if point.y.is_nan() {
                None
            } else {
                Some(point.y)
            });
        }

        PlotData {
            timestamps,
            series_data: vec![series_data],
        }
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        // Reset if the session changed.
        let mut need_reset = self
            .last_session_id
            .map_or(false, |sid| sid != batch.session_id);

        // Reset if sample numbers went backwards (wrap/reset).
        if !need_reset {
            if let (Some(prev_sn), Some(first_sn)) = (
                self.last_sample_number,
                batch.sample_numbers.first().copied(),
            ) {
                if first_sn < prev_sn {
                    need_reset = true;
                }
            }
        }

        // Reset if (for any reason) time went backwards.
        if !need_reset && batch.t_max < self.last_processed_time {
            need_reset = true;
        }

        if need_reset {
            // Preserve output buffer (old data stays visible), reset algorithm only
            self.reset_algorithm_state();
            // Insert gap marker so the chart shows a break
            if let Some(first_point) = batch.points.first() {
                self.retain_point(Point::new(first_point.x, f64::NAN));
            }
        }

        for point in batch.points.iter() {
            self.process_point(*point);
        }

        self.last_processed_time = batch.t_max;
        self.last_session_id = Some(batch.session_id);
        self.last_sample_number = batch.sample_numbers.last().copied();
    }

    fn process_command(&mut self, cmd: PipelineCommand) {
        match cmd {
            PipelineCommand::ResetSelf => self.reset_internal(),
            PipelineCommand::Hydrate {
                sampling_rate,
                backfill,
            } => {
                if self.capacity == 0 && self.window_seconds > 0.0 {
                    if let Some(rate) = sampling_rate {
                        if rate > 0.0 {
                            let output_rate_approx = (2.0 * rate) / self.ratio as f64;
                            let new_capacity =
                                (output_rate_approx * self.window_seconds).ceil() as usize;
                            self.capacity = new_capacity.max(2);
                        }
                    }
                }

                if backfill.is_empty() {
                    return;
                }

                for point in &backfill {
                    self.process_point(*point);
                }
                if let Some(last) = backfill.last() {
                    self.last_processed_time = last.x;
                }
            }
            _ => {}
        }
    }
}
