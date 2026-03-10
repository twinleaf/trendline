use super::{Pipeline, PipelineCommand};
use crate::shared::{MathOp, PipelineId, PlotData, Point, UiColumnKey};
use crate::state::data_manager::BatchedData;
use crossbeam::channel::Sender;
use std::collections::VecDeque;
use std::sync::Arc;
use uuid::Uuid;

const BUF_CAP: usize = 4096;

pub struct ArithmeticProducer {
    id: PipelineId,
    a: UiColumnKey,
    b: UiColumnKey,
    op: MathOp,
    out_key: UiColumnKey,
    virtual_tx: Sender<Arc<BatchedData>>,

    buf_a: VecDeque<Point>,
    buf_b: VecDeque<Point>,
    last_session_a: Option<u32>,
    last_session_b: Option<u32>,
}

impl ArithmeticProducer {
    pub fn new(
        a: UiColumnKey,
        b: UiColumnKey,
        op: MathOp,
        out_key: UiColumnKey,
        virtual_tx: Sender<Arc<BatchedData>>,
    ) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            a,
            b,
            op,
            out_key,
            virtual_tx,
            buf_a: VecDeque::with_capacity(BUF_CAP),
            buf_b: VecDeque::with_capacity(BUF_CAP),
            last_session_a: None,
            last_session_b: None,
        }
    }

    fn ingest(buf: &mut VecDeque<Point>, pts: &[Point]) {
        for p in pts {
            if buf.len() >= BUF_CAP {
                buf.pop_front();
            }
            buf.push_back(*p);
        }
    }

    fn interp(buf: &VecDeque<Point>, t: f64) -> Option<f64> {
        if buf.is_empty() {
            return None;
        }

        let mut last: Option<Point> = None;
        let mut next: Option<Point> = None;

        for p in buf.iter().rev() {
            if p.x <= t {
                last = Some(*p);
                break;
            }
        }
        for p in buf.iter() {
            if p.x >= t {
                next = Some(*p);
                break;
            }
        }

        match (last, next) {
            (Some(p1), Some(p2)) if (p2.x - p1.x).abs() > 1e-12 => {
                Some(p1.y + (p2.y - p1.y) * (t - p1.x) / (p2.x - p1.x))
            }
            (Some(p1), _) => Some(p1.y),
            (_, Some(p2)) => Some(p2.y),
            _ => None,
        }
    }

    fn apply(op: &MathOp, a: f64, b: f64) -> f64 {
        match op {
            MathOp::Add => a + b,
            MathOp::Sub => a - b,
            MathOp::Mul => a * b,
            MathOp::Div => {
                if b.abs() < 1e-15 {
                    f64::NAN
                } else {
                    a / b
                }
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

    fn process_side(&mut self, incoming_key: &UiColumnKey, batch: Arc<BatchedData>) {
        let pts = &*batch.points;
        let sns = &*batch.sample_numbers;

        if &self.a == incoming_key {
            if self.last_session_a != Some(batch.session_id) {
                self.buf_a.clear();
                self.last_session_a = Some(batch.session_id);
            }
            Self::ingest(&mut self.buf_a, pts);
            if self.buf_b.is_empty() {
                return;
            }
            let mut out_pts = Vec::with_capacity(pts.len());
            let mut out_sns = Vec::with_capacity(pts.len());
            for (i, p) in pts.iter().enumerate() {
                if let Some(b_y) = Self::interp(&self.buf_b, p.x) {
                    let y = Self::apply(&self.op, p.y, b_y);
                    out_pts.push(Point { x: p.x, y });
                    out_sns.push(sns[i]);
                }
            }
            self.emit_batch(out_pts, out_sns, batch.session_id);
        } else {
            if self.last_session_b != Some(batch.session_id) {
                self.buf_b.clear();
                self.last_session_b = Some(batch.session_id);
            }
            Self::ingest(&mut self.buf_b, pts);
            if self.buf_a.is_empty() {
                return;
            }
            let mut out_pts = Vec::with_capacity(pts.len());
            for p in pts.iter() {
                if let Some(a_y) = Self::interp(&self.buf_a, p.x) {
                    let y = Self::apply(&self.op, a_y, p.y);
                    out_pts.push(Point { x: p.x, y });
                }
            }
            self.emit_batch(out_pts, sns.to_vec(), batch.session_id);
        }
    }
}

impl Pipeline for ArithmeticProducer {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData {
        PlotData::empty()
    }

    fn process_batch(&mut self, batch: Arc<BatchedData>) {
        let is_relevant = batch.key == self.a || batch.key == self.b;
        if !is_relevant {
            return;
        }
        let incoming_key = batch.key.clone();
        self.process_side(&incoming_key, batch);
    }

    fn process_command(&mut self, cmd: PipelineCommand) {
        match cmd {
            PipelineCommand::ResetSelf => {
                self.buf_a.clear();
                self.buf_b.clear();
                self.last_session_a = None;
                self.last_session_b = None;
            }
            PipelineCommand::Hydrate { .. } => {
                // Rate computation is handled by PipelineModel before spawning
            }
            _ => {}
        }
    }
}
