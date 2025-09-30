use crate::pipeline::StatisticsProvider;
use crate::shared::{DataColumnId, PipelineId, Point, StatisticSet, StreamStatistics};
use crate::state::capture::{BatchedData, CaptureState, SessionId};
use crate::util::calculate_batch_stats;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
struct PersistentCalculator {
    count: u64,
    nan_count: u64,
    mean: f64,
    m2: f64,
    sum_of_squares: f64,
    min: f64,
    max: f64,
}

impl PersistentCalculator {
    fn new(first_value: f64) -> Self {
        if first_value.is_finite() {
            Self {
                count: 1,
                nan_count: 0,
                mean: first_value,
                m2: 0.0,
                sum_of_squares: first_value.powi(2),
                min: first_value,
                max: first_value,
            }
        } else {
            Self {
                count: 0,
                nan_count: 1,
                mean: 0.0,
                m2: 0.0,
                sum_of_squares: 0.0,
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            }
        }
    }
    fn update(&mut self, new_value: f64) {
        if !new_value.is_finite() {
            self.nan_count += 1;
            return;
        }

        if self.count == 0 {
            self.count = 1;
            self.mean = new_value;
            self.m2 = 0.0;
            self.sum_of_squares = new_value.powi(2);
            self.min = new_value;
            self.max = new_value;
            return;
        }

        self.count += 1;

        let delta = new_value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = new_value - self.mean;
        self.m2 += delta * delta2;

        self.sum_of_squares += new_value.powi(2);

        if new_value < self.min {
            self.min = new_value;
        }
        if new_value > self.max {
            self.max = new_value;
        }
    }

    fn stdev(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            (self.m2 / (self.count - 1) as f64).sqrt()
        }
    }

    fn rms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.sum_of_squares / self.count as f64).sqrt()
        }
    }

    fn to_statistic_set(&self) -> StatisticSet {
        let (min, max) = if self.count > 0 {
            (self.min, self.max)
        } else {
            (0.0, 0.0)
        };

        StatisticSet {
            count: self.count,
            nan_count: self.nan_count,
            mean: self.mean,
            min,
            max,
            stdev: self.stdev(),
            rms: self.rms(),
        }
    }
}

pub struct StreamingStatisticsProvider {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,

    buf: VecDeque<Point>,
    scratch: Vec<Point>,

    persistent_calculator: Option<PersistentCalculator>,

    last_session_id: Option<SessionId>,

    output: Arc<Mutex<StreamStatistics>>,
}

impl StreamingStatisticsProvider {
    pub fn new(source_key: DataColumnId, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            buf: VecDeque::new(),
            scratch: Vec::with_capacity(4096),
            persistent_calculator: None,
            last_session_id: None,
            output: Arc::new(Mutex::new(StreamStatistics::default())),
        }
    }

    #[inline]
    fn trim_window(&mut self, t_now: f64) {
        let t_min = t_now - self.window_seconds;
        while let Some(front) = self.buf.front() {
            if front.x < t_min {
                self.buf.pop_front();
            } else {
                break;
            }
        }
    }

    fn recompute_window_stats(&mut self) -> StatisticSet {
        self.scratch.clear();
        // Copy refs as Points (cheap: Point is two f64s)
        self.scratch.extend(self.buf.iter().copied());
        calculate_batch_stats(&self.scratch)
    }
}

impl StatisticsProvider for StreamingStatisticsProvider {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&mut self, _capture_state: &CaptureState) -> StreamStatistics {
        self.output.lock().unwrap().clone()
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        if batch.key != self.source_key {
            return;
        }

        if self
            .last_session_id
            .map_or(false, |sid| sid != batch.session_id)
        {
            self.buf.clear();
        }
        self.last_session_id = Some(batch.session_id);

        for p in batch.points.iter() {
            self.buf.push_back(*p);
            match &mut self.persistent_calculator {
                Some(calc) => calc.update(p.y),
                None => self.persistent_calculator = Some(PersistentCalculator::new(p.y)),
            }
        }

        if let Some(last) = self.buf.back().copied() {
            self.trim_window(last.x);

            let window_stats = self.recompute_window_stats();

            let persistent = self
                .persistent_calculator
                .as_ref()
                .map(|c| c.to_statistic_set())
                .unwrap_or_default();

            let snapshot = StreamStatistics {
                latest_value: last.y,
                window: window_stats,
                persistent,
            };
            *self.output.lock().unwrap() = snapshot;
        }
    }

    fn reset(&mut self, capture_state: &CaptureState) {
        self.buf.clear();
        self.scratch.clear();
        self.persistent_calculator = None;
        self.last_session_id = None;
        *self.output.lock().unwrap() = StreamStatistics::default();
        capture_state.clear_stream_by_key(&self.source_key);
    }
}
