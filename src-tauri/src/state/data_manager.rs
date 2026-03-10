use crate::shared::{Point, UiColumnKey};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use twinleaf::data::{AlignedWindow, Buffer, ReadError, Sample};
use twinleaf::tio::proto::identifiers::StreamKey;
use twinleaf::tio::proto::DeviceRoute;

pub type SessionId = u32;

#[derive(Clone)]
pub struct BatchedData {
    pub key: UiColumnKey,
    pub session_id: SessionId,
    pub points: Arc<Vec<Point>>,
    pub sample_numbers: Arc<Vec<u32>>,
    pub t_min: f64,
    pub t_max: f64,
}

pub enum DataMsg {
    Ingest {
        sample: Sample,
        route: DeviceRoute,
    },
    ReadWindow {
        columns: Vec<UiColumnKey>,
        n: usize,
        reply: Sender<Result<AlignedWindow, ReadError>>,
    },
    ReadTimeRange {
        columns: Vec<UiColumnKey>,
        start_time: f64,
        end_time: f64,
        reply: Sender<Result<AlignedWindow, ReadError>>,
    },
    CreateSnapshot {
        id: String,
        columns: Vec<UiColumnKey>,
        start_time: f64,
        end_time: f64,
        reply: Sender<Result<(), ReadError>>,
    },
    GetSnapshot {
        id: String,
        reply: Sender<Option<AlignedWindow>>,
    },
    ClearSnapshot {
        id: String,
    },
    Subscribe {
        key: UiColumnKey,
        id: usize,
        tx: Sender<Arc<BatchedData>>,
    },
    Unsubscribe {
        key: UiColumnKey,
        id: usize,
    },
    SetSamplingRate {
        key: UiColumnKey,
        rate: f64,
    },
    GetSamplingRate {
        key: UiColumnKey,
        reply: Sender<Option<f64>>,
    },
    Clear,
    Shutdown,
}

struct PendingBatch {
    session_id: SessionId,
    points: Vec<Point>,
    sample_numbers: Vec<u32>,
}

struct DataModel {
    capacity: usize,
    buffer: Buffer,
    snapshots: HashMap<String, AlignedWindow>,
    subscribers: HashMap<UiColumnKey, Vec<(usize, Sender<Arc<BatchedData>>)>>,
    sampling_rates: HashMap<UiColumnKey, f64>,
    pending: HashMap<UiColumnKey, PendingBatch>,
    session_ids: HashMap<StreamKey, SessionId>,
}

impl DataModel {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: Buffer::new(capacity),
            snapshots: HashMap::new(),
            subscribers: HashMap::new(),
            sampling_rates: HashMap::new(),
            pending: HashMap::new(),
            session_ids: HashMap::new(),
        }
    }

    fn accumulate_sample(&mut self, sample: &Sample, route: &DeviceRoute) {
        let stream_id = sample.stream.stream_id;
        let timestamp = sample.timestamp_begin();
        let sample_number = sample.n;
        let stream_key = StreamKey::new(route.clone(), stream_id);

        // Detect session boundary
        let is_boundary = sample.boundary.as_ref().is_some_and(|b| !b.is_continuous());
        if is_boundary {
            // Flush any pending data for columns in this stream before session change
            self.flush_stream(route, stream_id);
            // Increment session counter
            let session = self.session_ids.entry(stream_key.clone()).or_insert(0);
            *session = session.wrapping_add(1);
        }

        let session_id = self
            .session_ids
            .get(&stream_key)
            .copied()
            .unwrap_or_else(|| {
                // First sample for this stream — use device session_id as initial value
                let sid = sample.device.session_id;
                self.session_ids.insert(stream_key, sid);
                sid
            });

        for (col_idx, col) in sample.columns.iter().enumerate() {
            let Some(value) = col.value.try_as_f64() else {
                continue;
            };
            let key = UiColumnKey::new(route.clone(), stream_id, col_idx);

            // Skip columns with no subscribers
            if !self.subscribers.contains_key(&key) {
                continue;
            }

            let pending = self.pending.entry(key).or_insert_with(|| PendingBatch {
                session_id,
                points: Vec::with_capacity(64),
                sample_numbers: Vec::with_capacity(64),
            });

            if pending.session_id != session_id {
                continue;
            }

            pending.points.push(Point {
                x: timestamp,
                y: value,
            });
            pending.sample_numbers.push(sample_number);
        }
    }

    fn flush_stream(&mut self, route: &DeviceRoute, stream_id: u8) {
        let keys_to_flush: Vec<UiColumnKey> = self
            .pending
            .keys()
            .filter(|k| k.route == *route && k.stream_id == stream_id)
            .cloned()
            .collect();

        for key in keys_to_flush {
            self.flush_column(&key);
        }
    }

    fn flush_all(&mut self) {
        let keys: Vec<UiColumnKey> = self.pending.keys().cloned().collect();
        for key in keys {
            self.flush_column(&key);
        }
    }

    fn flush_column(&mut self, key: &UiColumnKey) {
        let Some(batch) = self.pending.remove(key) else {
            return;
        };
        if batch.points.is_empty() {
            return;
        }

        let subscribers = match self.subscribers.get(key) {
            Some(subs) if !subs.is_empty() => subs,
            _ => return,
        };

        let t_min = batch.points.first().map(|p| p.x).unwrap_or(0.0);
        let t_max = batch.points.last().map(|p| p.x).unwrap_or(0.0);

        let batched = Arc::new(BatchedData {
            key: key.clone(),
            session_id: batch.session_id,
            points: Arc::new(batch.points),
            sample_numbers: Arc::new(batch.sample_numbers),
            t_min,
            t_max,
        });

        for (_, tx) in subscribers {
            let _ = tx.try_send(batched.clone());
        }
    }

    fn clear(&mut self) {
        self.buffer = Buffer::new(self.capacity);
        self.snapshots.clear();
        self.pending.clear();
        self.session_ids.clear();
        // Keep subscribers and sampling_rates — they survive reconnection
    }
}

pub struct DataManager {
    tx: Sender<DataMsg>,
}

impl Clone for DataManager {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl DataManager {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = unbounded::<DataMsg>();
        Self::spawn_actor(capacity, rx);
        Self { tx }
    }

    fn spawn_actor(capacity: usize, rx: Receiver<DataMsg>) {
        thread::Builder::new()
            .name("data-manager".into())
            .spawn(move || {
                let mut model = DataModel::new(capacity);

                while let Ok(msg) = rx.recv() {
                    if Self::handle_msg(&mut model, msg, &rx) {
                        break;
                    }
                }
            })
            .expect("Failed to spawn data-manager thread.");
    }

    /// Returns `true` if the actor should stop.
    fn handle_msg(model: &mut DataModel, msg: DataMsg, rx: &Receiver<DataMsg>) -> bool {
        match msg {
            DataMsg::Shutdown => return true,
            DataMsg::Ingest { sample, route } => {
                model.accumulate_sample(&sample, &route);
                let stream_key = StreamKey::new(route, sample.stream.stream_id);
                model.buffer.process_sample(sample, stream_key);

                // Drain any more Ingest messages available right now
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        DataMsg::Ingest { sample, route } => {
                            model.accumulate_sample(&sample, &route);
                            let sk = StreamKey::new(route, sample.stream.stream_id);
                            model.buffer.process_sample(sample, sk);
                        }
                        DataMsg::Shutdown => {
                            model.flush_all();
                            return true;
                        }
                        other => {
                            model.flush_all();
                            Self::handle_non_ingest(model, other);
                            break;
                        }
                    }
                }

                model.flush_all();
                false
            }
            other => {
                Self::handle_non_ingest(model, other);
                false
            }
        }
    }

    fn handle_non_ingest(model: &mut DataModel, msg: DataMsg) {
        match msg {
            DataMsg::Ingest { .. } | DataMsg::Shutdown => unreachable!(),
            DataMsg::ReadWindow { columns, n, reply } => {
                let keys: Vec<_> = columns.iter().map(Into::into).collect();
                let result = model.buffer.read_aligned_window(&keys, n);
                let _ = reply.send(result);
            }
            DataMsg::ReadTimeRange {
                columns,
                start_time,
                end_time,
                reply,
            } => {
                let keys: Vec<_> = columns.iter().map(Into::into).collect();
                let result = model
                    .buffer
                    .read_aligned_time_range(&keys, start_time, end_time);
                let _ = reply.send(result);
            }
            DataMsg::CreateSnapshot {
                id,
                columns,
                start_time,
                end_time,
                reply,
            } => {
                let keys: Vec<_> = columns.iter().map(Into::into).collect();
                let result = model
                    .buffer
                    .read_aligned_time_range(&keys, start_time, end_time);
                let _ = reply.send(result.map(|window| {
                    model.snapshots.insert(id, window);
                }));
            }
            DataMsg::GetSnapshot { id, reply } => {
                let _ = reply.send(model.snapshots.get(&id).cloned());
            }
            DataMsg::ClearSnapshot { id } => {
                model.snapshots.remove(&id);
            }
            DataMsg::Subscribe { key, id, tx } => {
                model.subscribers.entry(key).or_default().push((id, tx));
            }
            DataMsg::Unsubscribe { key, id } => {
                if let Some(subs) = model.subscribers.get_mut(&key) {
                    subs.retain(|(sub_id, _)| *sub_id != id);
                    if subs.is_empty() {
                        model.subscribers.remove(&key);
                    }
                }
            }
            DataMsg::SetSamplingRate { key, rate } => {
                model.sampling_rates.insert(key, rate);
            }
            DataMsg::GetSamplingRate { key, reply } => {
                let _ = reply.send(model.sampling_rates.get(&key).copied());
            }
            DataMsg::Clear => {
                model.clear();
            }
        }
    }

    pub fn dispatch(&self, msg: DataMsg) -> Result<(), String> {
        self.tx
            .send(msg)
            .map_err(|_| "DataManager actor is unavailable.".to_string())
    }

    pub fn sender(&self) -> Sender<DataMsg> {
        self.tx.clone()
    }

    pub fn ask<R>(&self, build: impl FnOnce(Sender<R>) -> DataMsg) -> Result<R, String>
    where
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = bounded(1);
        self.tx
            .send(build(reply_tx))
            .map_err(|_| "DataManager actor is unavailable.".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "DataManager actor did not respond.".to_string())
    }
}
