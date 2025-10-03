use crate::pipeline::StatisticsProvider;
use crate::shared::{ColumnStatistics, DataColumnId, HealthSet, PipelineId, Point, StatisticSet};
use crate::state::capture::{BatchedData, CaptureState, SessionId};
use crate::util::{calculate_health_stats, calculate_value_stats};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
struct ValueAccumulator {
    count: u64,
    mean: f64,
    m2: f64,
    sum_of_squares: f64,
    min: f64,
    max: f64,
}

impl ValueAccumulator {
    fn update(&mut self, v: f64) {
        if !v.is_finite() { return; }
        if self.count == 0 {
            self.count = 1;
            self.mean = v;
            self.m2 = 0.0;
            self.sum_of_squares = v * v;
            self.min = v;
            self.max = v;
            return;
        }
        self.count += 1;
        let d = v - self.mean;
        self.mean += d / self.count as f64;
        let d2 = v - self.mean;
        self.m2 += d * d2;
        self.sum_of_squares += v * v;
        if v < self.min { self.min = v; }
        if v > self.max { self.max = v; }
    }
    fn to_stat(&self) -> StatisticSet {
        if self.count == 0 {
            return StatisticSet::default();
        }
        let stdev = if self.count > 1 {
            (self.m2 / (self.count - 1) as f64).sqrt()
        } else { 0.0 };
        let rms = (self.sum_of_squares / self.count as f64).sqrt();
        StatisticSet {
            count: self.count,
            mean: self.mean,
            min: self.min,
            max: self.max,
            stdev,
            rms,
        }
    }
    fn reset(&mut self) { *self = Self::default(); }
}

#[derive(Clone, Debug, Default)]
struct HealthAccumulator {
    gap_count: u64,
    gap_sum: f64,
    gap_min: f64,
    gap_max: f64,
    nan_count: u64,
}

impl HealthAccumulator {
    fn update(&mut self, prev_sn: Option<u32>, sn: u32, y: f64) -> Option<u32> {
        if !y.is_finite() {
            self.nan_count += 1;
        }
        if let Some(p) = prev_sn {
            if sn > p {
                let d: u32 = sn - p;
                if d > 1 {
                    let gap = f64::from(d - 1);
                    self.gap_count += 1;
                    self.gap_sum += gap;
                    if self.gap_count == 1 {
                        self.gap_min = gap;
                        self.gap_max = gap;
                    } else {
                        if gap < self.gap_min { self.gap_min = gap; }
                        if gap > self.gap_max { self.gap_max = gap; }
                    }
                }
            }
            // else: out-of-order or duplicate; ignore for gaps
        }
        Some(sn)
    }
    fn to_health(&self) -> HealthSet {
        let (gap_mean, gap_min, gap_max) = if self.gap_count == 0 {
            (0.0, 0.0, 0.0)
        } else {
            (self.gap_sum / self.gap_count as f64, self.gap_min, self.gap_max)
        };
        HealthSet {
            gap_count: self.gap_count,
            nan_count: self.nan_count,
            gap_mean,
            gap_min,
            gap_max,
        }
    }
    fn reset(&mut self) { *self = Self::default(); }
}

pub struct StreamingStatisticsProvider {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,

    buf_points: VecDeque<Point>,
    buf_samples: VecDeque<u32>,
    scratch_points: Vec<Point>,
    scratch_samples: Vec<u32>,

    persistent_values: ValueAccumulator,
    persistent_health: HealthAccumulator,

    last_session_id: Option<SessionId>,
    last_sample_number: Option<u32>,

    output: Arc<Mutex<ColumnStatistics>>,
}

impl StreamingStatisticsProvider {
    pub fn new(source_key: DataColumnId, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            buf_points: VecDeque::new(),
            buf_samples: VecDeque::new(),
            scratch_points: Vec::with_capacity(4096),
            scratch_samples: Vec::with_capacity(4096),
            persistent_values: ValueAccumulator::default(),
            persistent_health: HealthAccumulator::default(),
            last_session_id: None,
            last_sample_number: None,
            output: Arc::new(Mutex::new(ColumnStatistics::default())),
        }
    }

    #[inline]
    fn trim_window(&mut self, t_now: f64) {
        let t_min = t_now - self.window_seconds;
        while let (Some(p), Some(_s)) = (self.buf_points.front(), self.buf_samples.front()) {
            if p.x < t_min {
                self.buf_points.pop_front();
                self.buf_samples.pop_front();
            } else {
                break;
            }
        }
    }

    fn recompute_window_value_stats(&mut self) -> StatisticSet {
        self.scratch_points.clear();
        self.scratch_points.extend(self.buf_points.iter().copied());
        calculate_value_stats(&self.scratch_points)
    }

    fn recompute_window_health(&mut self) -> HealthSet {
        self.scratch_points.clear();
        self.scratch_samples.clear();
        self.scratch_points.extend(self.buf_points.iter().copied());
        self.scratch_samples.extend(self.buf_samples.iter().copied());
        calculate_health_stats(&self.scratch_samples, &self.scratch_points)
    }
}

impl StatisticsProvider for StreamingStatisticsProvider {
    fn id(&self) -> PipelineId { self.id }

    fn get_output(&mut self, _capture_state: &CaptureState) -> ColumnStatistics {
        self.output.lock().unwrap().clone()
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        if batch.key != self.source_key {
            return;
        }

        if self.last_session_id.map_or(false, |sid| sid != batch.session_id) {
            self.buf_points.clear();
            self.buf_samples.clear();
            self.last_sample_number = None;
        }
        self.last_session_id = Some(batch.session_id);

        debug_assert_eq!(
            batch.points.len(), batch.sample_numbers.len(),
            "points and sample_numbers must be same length"
        );

        let n = batch.points.len().min(batch.sample_numbers.len());
        for i in 0..n {
            let p = batch.points[i];
            let sn = batch.sample_numbers[i];

            self.buf_points.push_back(p);
            self.buf_samples.push_back(sn);

            self.persistent_values.update(p.y);
            self.last_sample_number = self.persistent_health.update(self.last_sample_number, sn, p.y);
        }

        if let Some(last) = self.buf_points.back().copied() {
            self.trim_window(last.x);

            let window_stats = self.recompute_window_value_stats();
            let window_health = self.recompute_window_health();

            let persistent = self.persistent_values.to_stat();
            let persistent_health = self.persistent_health.to_health();

            let snapshot = ColumnStatistics {
                latest_value: last.y,
                persistent,
                window: window_stats,
                persistent_health,
                window_health,
            };
            *self.output.lock().unwrap() = snapshot;
        }
    }

    fn reset(&mut self) {
        self.buf_points.clear();
        self.buf_samples.clear();
        self.scratch_points.clear();
        self.scratch_samples.clear();
        self.persistent_values.reset();
        self.persistent_health.reset();
        self.last_session_id = None;
        self.last_sample_number = None;
        *self.output.lock().unwrap() = ColumnStatistics::default();
    }
}