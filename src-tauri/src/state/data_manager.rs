use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::thread;
use twinleaf::data::{AlignedWindow, Buffer, ReadError, Sample};
use twinleaf::tio::proto::identifiers::{ColumnKey, StreamKey};
use twinleaf::tio::proto::DeviceRoute;

pub enum DataMsg {
    Ingest {
        sample: Sample,
        route: DeviceRoute,
    },
    ReadWindow {
        columns: Vec<ColumnKey>,
        n: usize,
        reply: Sender<Result<AlignedWindow, ReadError>>,
    },
    ReadTimeRange {
        columns: Vec<ColumnKey>,
        start_time: f64,
        end_time: f64,
        reply: Sender<Result<AlignedWindow, ReadError>>,
    },
    CreateSnapshot {
        id: String,
        columns: Vec<ColumnKey>,
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
    Shutdown,
}

struct DataModel {
    buffer: Buffer,
    snapshots: HashMap<String, AlignedWindow>,
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
                let mut model = DataModel {
                    buffer: Buffer::new(capacity),
                    snapshots: HashMap::new(),
                };

                while let Ok(msg) = rx.recv() {
                    match msg {
                        DataMsg::Ingest { sample, route } => {
                            let stream_key = StreamKey::new(route, sample.stream.stream_id);
                            model.buffer.process_sample(sample, stream_key);
                        }
                        DataMsg::ReadWindow { columns, n, reply } => {
                            let result = model.buffer.read_aligned_window(&columns, n);
                            let _ = reply.send(result);
                        }
                        DataMsg::ReadTimeRange {
                            columns,
                            start_time,
                            end_time,
                            reply,
                        } => {
                            let result = model
                                .buffer
                                .read_aligned_time_range(&columns, start_time, end_time);
                            let _ = reply.send(result);
                        }
                        DataMsg::CreateSnapshot {
                            id,
                            columns,
                            start_time,
                            end_time,
                            reply,
                        } => {
                            let result = model
                                .buffer
                                .read_aligned_time_range(&columns, start_time, end_time);
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
                        DataMsg::Shutdown => break,
                    }
                }
            })
            .expect("Failed to spawn data-manager thread.");
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
