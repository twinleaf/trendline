use crate::pipeline::StatisticsProvider;
use crate::shared::{DataColumnId, StatisticSet, StreamStatistics, PipelineId};
use crate::state::capture::CaptureState;
use crate::util::calculate_batch_stats;
use crate::pipeline::buffer::DoubleBuffer;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

#[derive(Clone, Debug, Default)]
struct PersistentCalculator {
    count: u64,
    mean: f64,
    m2: f64,
    sum_of_squares: f64,
    min: f64,
    max: f64,
}

impl PersistentCalculator {
    fn new(first_value: f64) -> Self {
        Self {
            count: 1,
            mean: first_value,
            m2: 0.0,
            sum_of_squares: first_value.powi(2),
            min: first_value,
            max: first_value,
        }
    }

    fn update(&mut self, new_value: f64) {
        self.count += 1;
        let delta = new_value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = new_value - self.mean;
        self.m2 += delta * delta2;
        self.sum_of_squares += new_value.powi(2);

        if new_value < self.min { self.min = new_value; }
        if new_value > self.max { self.max = new_value; }
    }
    
    fn stdev(&self) -> f64 {
        if self.count < 2 { 0.0 } else { (self.m2 / (self.count - 1) as f64).sqrt() }
    }

    fn rms(&self) -> f64 {
        if self.count == 0 { 0.0 } else { (self.sum_of_squares / self.count as f64).sqrt() }
    }
    
    fn to_statistic_set(&self) -> StatisticSet {
        StatisticSet {
            count: self.count,
            mean: self.mean,
            min: self.min,
            max: self.max,
            stdev: self.stdev(),
            rms: self.rms(),
        }
    }
}

pub struct StreamingStatisticsProvider {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,
    output: Arc<Mutex<DoubleBuffer<StreamStatistics>>>,
    last_processed_time: f64,
    persistent_calculator: Option<PersistentCalculator>,
}

impl StreamingStatisticsProvider {
    pub fn new(source_key: DataColumnId, window_seconds: f64) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            output: Arc::new(Mutex::new(DoubleBuffer::new())),
            last_processed_time: 0.0,
            persistent_calculator: None,
        }
    }
}

impl StatisticsProvider for StreamingStatisticsProvider {
    fn id(&self) -> PipelineId {
        self.id
    }
    fn get_output(&self) -> StreamStatistics {
        self.output.lock().unwrap().read_with(|data| data.clone())
    }

    fn update(&mut self, capture_state: &CaptureState) {
        let Some(latest_time) = capture_state.get_latest_unified_timestamp(&[self.source_key.clone()]) else {
            return;
        };

        let new_points_vec = capture_state.get_data_across_sessions_for_keys(
            &[self.source_key.clone()],
            self.last_processed_time,
            latest_time,
        );
        if let Some(points) = new_points_vec.get(0) {
            for point in points {
                match &mut self.persistent_calculator {
                    Some(calc) => calc.update(point.y),
                    None => self.persistent_calculator = Some(PersistentCalculator::new(point.y)),
                }
            }
        }
        
        let window_min_time = latest_time - self.window_seconds;
        let window_points_vec = capture_state.get_data_across_sessions_for_keys(
            &[self.source_key.clone()],
            window_min_time,
            latest_time,
        );

        let mut new_stats = StreamStatistics::default();

        if let Some(points) = window_points_vec.get(0) {
            if !points.is_empty() {
                if let Some(last_point) = points.last() {
                    new_stats.latest_value = last_point.y;
                }
                new_stats.window = calculate_batch_stats(points);
            }
        }

        if let Some(calc) = &self.persistent_calculator {
            new_stats.persistent = calc.to_statistic_set();
        }
        
        self.output.lock().unwrap().write_with(|stats_back_buffer| {
            *stats_back_buffer = new_stats;
        });
        
        self.last_processed_time = latest_time;
    }

    fn reset(&mut self, capture_state: &CaptureState) {
        self.persistent_calculator = None;

        let latest_time = capture_state
            .get_latest_unified_timestamp(&[self.source_key.clone()])
            .unwrap_or(0.0);
            
        self.last_processed_time = latest_time;

        self.output.lock().unwrap().write_with(|b| *b = StreamStatistics::default());
        
        println!("[Stats Provider {:?}] Reset. New start time: {}", self.id, self.last_processed_time);
    }
}