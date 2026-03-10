use super::{Pipeline, PipelineCommand};
use crate::shared::{BiquadKind, BiquadParams, PipelineId, PlotData, Point, UiColumnKey};
use crate::state::data_manager::BatchedData;
use crossbeam::channel::Sender;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}
impl Biquad {
    fn new(kind: BiquadKind, params: &BiquadParams, fs: f64) -> Self {
        let f0 = params.cutoff_hz.max(1e-9).min(0.49 * fs); // avoid Nyquist weirdness
        let w0 = 2.0 * std::f64::consts::PI * (f0 / fs);
        let (sin, cos) = w0.sin_cos();
        let q = params.q.max(1e-6);

        let alpha = sin / (2.0 * q);
        let (mut b0, mut b1, mut b2, a0, mut a1, mut a2) = match kind {
            BiquadKind::Lowpass => {
                let k = 1.0 - cos;
                (
                    (k / 2.0),
                    k,
                    (k / 2.0),
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            BiquadKind::Highpass => {
                let k = 1.0 + cos;
                (
                    (k / 2.0),
                    -k,
                    (k / 2.0),
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            BiquadKind::Bandpass => {
                let b0 = alpha;
                (b0, 0.0, -b0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
        };

        // normalize
        b0 /= a0;
        b1 /= a0;
        b2 /= a0;
        a1 /= a0;
        a2 /= a0;

        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        // Transposed Direct Form II
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

pub struct BiquadProducer {
    id: PipelineId,
    src: UiColumnKey,
    out_key: UiColumnKey,
    kind: BiquadKind,
    params: BiquadParams,
    biquad: Option<Biquad>,
    fs: Option<f64>,
    last_session: Option<u32>,
    virtual_tx: Sender<Arc<BatchedData>>,
}

impl BiquadProducer {
    pub fn new(
        src: UiColumnKey,
        kind: BiquadKind,
        params: BiquadParams,
        out_key: UiColumnKey,
        virtual_tx: Sender<Arc<BatchedData>>,
    ) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            src,
            out_key,
            kind,
            params,
            biquad: None,
            fs: None,
            last_session: None,
            virtual_tx,
        }
    }

    fn ensure_biquad(&mut self) {
        if let Some(fs) = self.fs {
            if self.biquad.is_none() {
                let kind = self.kind.clone();
                let params = self.params.clone();
                self.biquad = Some(Biquad::new(kind, &params, fs));
            }
        }
    }

    fn emit_batch(&self, points: Vec<Point>, sample_numbers: Vec<u32>, session_id: u32) {
        if points.is_empty() {
            return;
        }
        let t_min = points.first().map(|p| p.x).unwrap_or(0.0);
        let t_max = points.last().map(|p| p.x).unwrap_or(0.0);
        let batch = Arc::new(BatchedData {
            key: self.out_key.clone(),
            session_id,
            points: Arc::new(points),
            sample_numbers: Arc::new(sample_numbers),
            t_min,
            t_max,
        });
        let _ = self.virtual_tx.try_send(batch);
    }
}

impl Pipeline for BiquadProducer {
    fn id(&self) -> PipelineId {
        self.id
    }
    fn get_output(&self) -> PlotData {
        PlotData::empty()
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        if batch.key != self.src {
            return;
        }
        if self.last_session != Some(batch.session_id) {
            if let Some(bq) = self.biquad.as_mut() {
                bq.reset();
            }
            self.last_session = Some(batch.session_id);
        }

        self.ensure_biquad();
        let Some(bq) = self.biquad.as_mut() else {
            return;
        };

        let pts = &*batch.points;
        let sns = &*batch.sample_numbers;

        let mut out_pts = Vec::with_capacity(pts.len());
        for p in pts.iter() {
            let y = bq.process(p.y);
            out_pts.push(Point { x: p.x, y });
        }

        self.emit_batch(out_pts, sns.to_vec(), batch.session_id);
    }

    fn process_command(&mut self, cmd: PipelineCommand) {
        match cmd {
            PipelineCommand::Hydrate {
                sampling_rate,
                backfill: _,
            } => {
                if let Some(fs) = sampling_rate {
                    self.fs = Some(fs);
                    let kind = self.kind.clone();
                    let params = self.params.clone();
                    self.biquad = Some(Biquad::new(kind, &params, fs));
                }
            }
            PipelineCommand::ResetSelf => {
                if let Some(bq) = self.biquad.as_mut() {
                    bq.reset();
                }
            }
            _ => {}
        }
    }
}
