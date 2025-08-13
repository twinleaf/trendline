use super::Pipeline;
use crate::pipeline::buffer::DoubleBuffer;
use crate::shared::{DataColumnId, DetrendMethod, PipelineId, PlotData};
use crate::state::capture::CaptureState;
use nalgebra::{DMatrix, DVector};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct DetrendPipeline {
    id: PipelineId,
    source_key: DataColumnId,
    window_seconds: f64,
    method: DetrendMethod,
    output: Arc<Mutex<DoubleBuffer<PlotData>>>,
}

impl DetrendPipeline {
    pub fn new(source_key: DataColumnId, window_seconds: f64, method: DetrendMethod) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            source_key,
            window_seconds,
            method,
            output: Arc::new(Mutex::new(DoubleBuffer::new())),
        }
    }
}

impl Pipeline for DetrendPipeline {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData {
        self.output.lock().unwrap().read_with(|data| data.clone())
    }

    fn update(&mut self, capture_state: &CaptureState) {
        let Some(latest_time) =
            capture_state.get_latest_unified_timestamp(&[self.source_key.clone()])
        else {
            return;
        };
        let min_time = latest_time - self.window_seconds;
        let raw_data_vecs = capture_state.get_data_across_sessions_for_keys(
            &[self.source_key.clone()],
            min_time,
            latest_time,
        );

        let Some(points) = raw_data_vecs.get(0) else {
            return;
        };
        if points.is_empty() {
            self.output
                .lock()
                .unwrap()
                .write_with(|b| *b = PlotData::empty());
            return;
        }

        let y_values: Vec<f64> = points.iter().map(|p| p.y).collect();
        let detrended_y = match self.method {
            DetrendMethod::None => remove_mean(&y_values),
            DetrendMethod::Linear => remove_linear_trend(&y_values),
            DetrendMethod::Quadratic => remove_quadratic_trend(&y_values),
        };

        self.output
            .lock()
            .unwrap()
            .write_with(|plot_data_back_buffer| {
                plot_data_back_buffer.timestamps = points.iter().map(|p| p.x).collect();
                plot_data_back_buffer.series_data = vec![detrended_y];
            });
    }

    fn get_source_sampling_rate(&self, capture_state: &CaptureState) -> Option<f64> {
        capture_state.get_effective_sampling_rate(&self.source_key)
    }
}

pub fn remove_mean(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n == 0 {
        return vec![];
    }
    let mean = y.iter().sum::<f64>() / n as f64;

    y.iter().map(|val| val - mean).collect()
}


pub fn remove_linear_trend(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n == 0 {
        return vec![];
    }

    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();

    let a = DMatrix::from_fn(n, 2, |r, c| if c == 0 { 1.0 } else { t[r] });
    let b = DVector::from_vec(y.to_vec());

    let coeffs = a.svd(true, true).solve(&b, 1e-10).unwrap();
    let c = coeffs[0];
    let m = coeffs[1];

    y.iter()
        .zip(t.iter())
        .map(|(yi, ti)| yi - (m * ti + c))
        .collect()
}

pub fn remove_quadratic_trend(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n < 3 {
        return remove_linear_trend(y);
    }

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

    y.iter()
        .zip(t.iter())
        .map(|(yi, ti)| yi - (a_coeff * ti.powi(2) + b_coeff * ti + c))
        .collect()
}
