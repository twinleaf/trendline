use super::{Pipeline, PipelineCommand};
use crate::shared::{DataColumnId, MathOp, PipelineId, PlotData, Point};
use crate::state::capture::CaptureCommand;
use crate::state::capture::{BatchedData, CaptureState};
use crossbeam::channel::Sender;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

const BUF_CAP: usize = 4096;

pub struct ArithmeticProducer {
    id: PipelineId,
    a: DataColumnId,
    b: DataColumnId,
    op: MathOp,
    out_key: DataColumnId,
    capture_tx: Sender<CaptureCommand>,

    buf_a: VecDeque<Point>,
    buf_b: VecDeque<Point>,
    last_session_a: Option<u32>,
    last_session_b: Option<u32>,
    out_rate: Option<f64>,
}

impl ArithmeticProducer {
    pub fn new(
        a: DataColumnId,
        b: DataColumnId,
        op: MathOp,
        out_key: DataColumnId,
        capture_tx: Sender<CaptureCommand>,
    ) -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            a,
            b,
            op,
            out_key,
            capture_tx,
            buf_a: VecDeque::with_capacity(BUF_CAP),
            buf_b: VecDeque::with_capacity(BUF_CAP),
            last_session_a: None,
            last_session_b: None,
            out_rate: None,
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

        // Find last p where p.x <= t
        let mut last: Option<Point> = None;
        let mut next: Option<Point> = None;

        for p in buf.iter().rev() {
            if p.x <= t {
                last = Some(*p);
                break;
            }
        }
        // find first p where p.x >= t by scanning forward; cheap with small buf
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
            (Some(p1), _) => Some(p1.y), // hold-last
            (_, Some(p2)) => Some(p2.y), // hold-next (startup)
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

    fn process_side(&mut self, incoming_key: &DataColumnId, batch: Arc<BatchedData>) {
        let pts = &*batch.points;
        let sns = &*batch.sample_numbers;

        // Reset buffer on session changes
        if &self.a == incoming_key {
            if self.last_session_a != Some(batch.session_id) {
                self.buf_a.clear();
                self.last_session_a = Some(batch.session_id);
            }
            Self::ingest(&mut self.buf_a, pts);
            // produce using B
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
            if !out_pts.is_empty() {
                let _ = self.capture_tx.send(CaptureCommand::InsertBatch {
                    key: self.out_key.clone(),
                    points: out_pts,
                    sample_numbers: out_sns,
                    session_id: batch.session_id,
                    instant: Instant::now(),
                });
            }
        } else {
            // incoming is B
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
            if !out_pts.is_empty() {
                let _ = self.capture_tx.send(CaptureCommand::InsertBatch {
                    key: self.out_key.clone(),
                    points: out_pts,
                    sample_numbers: sns.to_vec(),
                    session_id: batch.session_id,
                    instant: Instant::now(),
                });
            }
        }
    }
}

impl Pipeline for ArithmeticProducer {
    fn id(&self) -> PipelineId {
        self.id
    }

    // Producer pipelines don't emit PlotData directly.
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

    fn process_command(&mut self, cmd: PipelineCommand, capture: &CaptureState) {
        match cmd {
            PipelineCommand::ResetSelf => {
                self.buf_a.clear();
                self.buf_b.clear();
                self.last_session_a = None;
                self.last_session_b = None;
            }
            PipelineCommand::Hydrate => {
                let sr_a = capture.get_effective_sampling_rate(&self.a).unwrap_or(0.0);
                let sr_b = capture.get_effective_sampling_rate(&self.b).unwrap_or(0.0);
                let out = sr_a.max(sr_b).max(1.0);
                self.out_rate = Some(out);
                let _ = self.capture_tx.send(CaptureCommand::UpdateSampleRate {
                    key: self.out_key.clone(),
                    rate: out,
                });
            }
            _ => {}
        }
    }
}
