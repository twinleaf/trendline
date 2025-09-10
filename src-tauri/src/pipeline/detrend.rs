use super::{Pipeline, PipelineCommand};
use crate::shared::{DataColumnId, DetrendMethod, PipelineId, PlotData, Point};
use crate::state::capture::{BatchedData, CaptureState};
use crossbeam::channel::Sender;
use nalgebra::{DMatrix, DVector};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct DetrendPipeline {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,
    method: DetrendMethod,
    output: Arc<Mutex<PlotData>>,
    subscribers: Vec<Sender<(PlotData, f64)>>,
    buffer: VecDeque<Point>,
    window_size_samples: usize,
    hop_size_samples: usize,
    since_last_emit: usize,
    sample_rate: Option<f64>,
}
const HOP_TIME_SECONDS: f64 = 0.032;


impl DetrendPipeline {
    pub fn new(source_key: DataColumnId, window_seconds: f64, method: DetrendMethod) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            method,
            output: Arc::new(Mutex::new(PlotData::empty())),
            subscribers: Vec::new(),
            buffer: VecDeque::new(),
            window_size_samples: 0,
            hop_size_samples: 0,
            since_last_emit: 0,
            sample_rate: None,
        }
    }

    fn calculate_and_distribute(&mut self, block: &[Point]) {
        if block.is_empty() {
            return;
        }

        let y_values: Vec<f64> = block.iter().map(|p| p.y).collect();

        let detrended_y = match self.method {
            DetrendMethod::None => remove_mean(&y_values),
            DetrendMethod::Linear => remove_linear_trend(&y_values),
            DetrendMethod::Quadratic => remove_quadratic_trend(&y_values),
        };

        let result_plot_data = PlotData {
            timestamps: block.iter().map(|p| p.x).collect(),
            series_data: vec![detrended_y],
        };

        *self.output.lock().unwrap() = result_plot_data.clone();

        if let Some(sr) = self.sample_rate {
            for tx in &self.subscribers {
                let _ = tx.try_send((result_plot_data.clone(), sr));
            }
        }
    }
}

impl Pipeline for DetrendPipeline {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData {
        self.output.lock().unwrap().clone()
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        if batch.key != self.source_key || batch.points.is_empty() {
            return;
        }

        self.buffer.extend(batch.points.iter());

        let max_buffer_len = self.window_size_samples.saturating_add(self.hop_size_samples);
        if max_buffer_len > 0 && self.buffer.len() > max_buffer_len {
            let to_drain = self.buffer.len() - max_buffer_len;
            self.buffer.drain(..to_drain);
        }

        self.since_last_emit = self.since_last_emit.saturating_add(batch.points.len());

        if self.hop_size_samples == 0 || self.since_last_emit < self.hop_size_samples {
            return;
        }

        let have = self.buffer.len();
        let want = if self.window_size_samples == 0 {
            have
        } else {
            self.window_size_samples.min(have)
        };

        if want == 0 {
            self.since_last_emit = 0;
            return;
        }

        let slice: Vec<Point> = self
            .buffer
            .iter()
            .rev()
            .take(want)
            .rev()
            .cloned()
            .collect();

        self.calculate_and_distribute(&slice);

        if have >= self.window_size_samples && self.window_size_samples > 0 {
            self.buffer.drain(..self.hop_size_samples.min(self.buffer.len()));
        }

        self.since_last_emit = 0;
    }

    fn process_command(&mut self, cmd: PipelineCommand, capture_state: &CaptureState) {
        match cmd {
            PipelineCommand::AddSubscriber(tx) => {
                self.subscribers.push(tx);
            }
            PipelineCommand::Hydrate => {
                if let Some(sr) = capture_state.get_effective_sampling_rate(&self.source_key) {
                    self.sample_rate = Some(sr);
                    self.window_size_samples  = (sr * self.window_seconds).ceil() as usize;
                    self.hop_size_samples = ((sr * HOP_TIME_SECONDS).ceil() as usize).max(1);

                    println!(
                        "[Detrend {:?}] Hydrated. Window: {} samples, Hop: {} samples.",
                        self.id, self.window_size_samples, self.hop_size_samples
                    );
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

                if let Some(mut points) = raw_data_vecs.into_iter().next() {
                    if self.window_size_samples > 0 && points.len() > self.window_size_samples {
                        points.drain(..points.len() - self.window_size_samples);
                    }
                    self.calculate_and_distribute(&points);
                    self.buffer.extend(points);
                    self.since_last_emit = 0;
                }
            }
            _ => {}
        }
    }
}


pub fn remove_mean(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n == 0 { return vec![]; }
    let mean = y.iter().sum::<f64>() / n as f64;
    y.iter().map(|val| val - mean).collect()
}

pub fn remove_linear_trend(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n == 0 { return vec![]; }
    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let a = DMatrix::from_fn(n, 2, |r, c| if c == 0 { 1.0 } else { t[r] });
    let b = DVector::from_vec(y.to_vec());
    let coeffs = a.svd(true, true).solve(&b, 1e-10).unwrap();
    let c = coeffs[0];
    let m = coeffs[1];
    y.iter().zip(t.iter()).map(|(yi, ti)| yi - (m * ti + c)).collect()
}

pub fn remove_quadratic_trend(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n < 3 { return remove_linear_trend(y); }
    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let a = DMatrix::from_fn(n, 3, |r, c| match c {
        0 => 1.0,
        1 => t[r],
        _ => t[r].powi(2),
    });
    let b = DVector::from_vec(y.to_vec());
    let coeffs = a.svd(true, true).solve(&b, 1e-10).unwrap();
    let c = coeffs[0];
    let b_coeff = coeffs[1];
    let a_coeff = coeffs[2];
    y.iter().zip(t.iter()).map(|(yi, ti)| yi - (a_coeff * ti.powi(2) + b_coeff * ti + c)).collect()
}