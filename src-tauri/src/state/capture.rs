use crate::shared::{DataColumnId, PlotData, Point};
use crossbeam::channel::{bounded, Receiver, Sender};
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

pub type SessionId = u32;
pub type DeviceTime = f64;
pub type UnifiedTime = f64;
pub type TimeOffset = f64;

#[derive(Clone)]

pub struct Buffer {
    pub data: Arc<RwLock<BTreeMap<u64, f64>>>,
    pub cap: Arc<AtomicUsize>,
}

impl Buffer {
    fn new(cap: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(BTreeMap::new())),
            cap: Arc::new(AtomicUsize::new(cap)),
        }
    }

    fn push_many(&self, pts: &[Point]) {
        let mut map = self.data.write().unwrap();
        for p in pts {
            map.insert(p.x.to_bits(), p.y);
        }
        let cap = self.cap.load(Ordering::Relaxed);
        while map.len() > cap {
            if let Some(oldest) = map.keys().next().copied() {
                map.remove(&oldest);
            } else {
                break;
            }
        }
    }

    pub fn set_capacity(&self, new_cap: usize) {
        let old = self.cap.swap(new_cap, Ordering::Relaxed);
        if old == new_cap {
            return;
        }
        let mut map = self.data.write().unwrap();
        while map.len() > new_cap {
            if let Some(oldest) = map.keys().next().copied() {
                map.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub first_instant: Instant,
    pub last_instant: Instant,
    pub first_device_time: DeviceTime,
    pub last_device_time: DeviceTime,
}

#[derive(Debug, Default)]
pub struct StreamState {
    pub effective_sampling_rate: f64,
    pub session_meta: DashMap<SessionId, SessionMeta>,
    pub offsets_cache: Mutex<Option<Arc<HashMap<SessionId, TimeOffset>>>>,
}

#[derive(Clone)]
pub struct BatchedData {
    pub key: DataColumnId,
    pub session_id: SessionId,
    pub points: Arc<Vec<Point>>,
    pub t_min: f64,
    pub t_max: f64,
}

pub struct Inner {
    pub buffers: DashMap<DataColumnId, DashMap<SessionId, Buffer>>,
    pub streams: DashMap<DataColumnId, StreamState>,
    pub paused_snapshots: DashMap<String, PlotData>,
    pub active: DashMap<DataColumnId, ()>,
    pub command_tx: Sender<CaptureCommand>,
    pub subscribers: DashMap<DataColumnId, Vec<(usize, Sender<Arc<BatchedData>>)>>,
}
#[derive(Debug)]
pub enum CaptureCommand {
    InsertBatch {
        key: DataColumnId,
        points: Vec<Point>,
        session_id: SessionId,
        instant: Instant,
    },
    UpdateSampleRate {
        key: DataColumnId,
        rate: f64,
    },
    SetActiveColumns {
        port_url: String,
        keys_for_port: Vec<DataColumnId>,
    },
    CreateSnapshot {
        plot_id: String,
        keys: Vec<DataColumnId>,
        start_time: f64,
        end_time: f64,
    },
    ClearSnapshot {
        plot_id: String,
    },
    Subscribe {
        key: DataColumnId,
        id: usize,
        tx: Sender<Arc<BatchedData>>,
    },
    Unsubscribe {
        key: DataColumnId,
        id: usize,
    },
}

#[derive(Clone)]
pub struct CaptureState {
    pub inner: Arc<Inner>,
}

impl CaptureState {
    const BUFFER_WINDOW_SECONDS: f64 = 180.0;
    const DEFAULT_SAMPLING_RATE: f64 = 1000.0;

    pub fn new() -> Self {
        let (command_tx, command_rx) = bounded::<CaptureCommand>(8_192);
        let inner = Arc::new(Inner {
            buffers: DashMap::new(),
            streams: DashMap::new(),
            paused_snapshots: DashMap::new(),
            active: DashMap::new(),
            command_tx,
            subscribers: DashMap::new(),
        });

        let consumer_inner = inner.clone();
        thread::Builder::new()
            .name("capture-consumer".into())
            .spawn(move || Self::run_consumer(consumer_inner, command_rx))
            .expect("Failed to spawn CaptureState thread.");

        Self { inner }
    }

    pub fn get_latest_unified_timestamp(&self, keys: &[DataColumnId]) -> Option<UnifiedTime> {
        let unique_stream_keys: std::collections::HashSet<_> =
            keys.iter().map(|k| k.stream_key()).collect();

        let all_offsets = unique_stream_keys
            .iter()
            .filter_map(|stream_key| {
                self._get_or_compute_offsets_for_stream(stream_key)
                    .map(|offsets| (stream_key.clone(), offsets))
            })
            .collect::<HashMap<_, _>>();

        keys.iter()
            .filter_map(|key| {
                let offsets = all_offsets.get(&key.stream_key())?;

                let (latest_session_id, max_offset) = offsets
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?;

                let binding = self.inner.buffers.get(key)?;
                let buffer = binding.get(latest_session_id)?;

                let last_raw_ts_bits = {
                    let map = buffer.data.read().unwrap();
                    map.iter().next_back().map(|(k, _)| *k)
                }?;

                let last_raw_ts = DeviceTime::from_bits(last_raw_ts_bits);
                let unified_ts = last_raw_ts + max_offset;
                Some(unified_ts)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn get_data_across_sessions_for_keys(
        &self,
        keys: &[DataColumnId],
        start_time: UnifiedTime,
        end_time: UnifiedTime,
    ) -> Vec<Vec<Point>> {
        let unique_stream_keys: std::collections::HashSet<_> =
            keys.iter().map(|k| k.stream_key()).collect();
        let all_offsets = unique_stream_keys
            .iter()
            .filter_map(|stream_key| {
                self._get_or_compute_offsets_for_stream(stream_key)
                    .map(|offsets| (stream_key.clone(), offsets))
            })
            .collect::<HashMap<_, _>>();

        keys.par_iter()
            .map(|key| {
                let Some(offsets) = all_offsets.get(&key.stream_key()) else {
                    return vec![];
                };
                let Some(stream_state) = self.inner.streams.get(&key.stream_key()) else {
                    return vec![];
                };
                let Some(session_map) = self.inner.buffers.get(key) else {
                    return vec![];
                };

                let mut sorted_sessions: Vec<_> = stream_state
                    .session_meta
                    .iter()
                    .filter_map(|entry| {
                        let session_id = *entry.key();
                        let meta = entry.value();
                        offsets
                            .get(&session_id)
                            .map(|offset| (session_id, meta.clone(), *offset))
                    })
                    .collect();

                sorted_sessions.sort_by(|a, b| {
                    let start_a = a.1.first_device_time + a.2;
                    let start_b = b.1.first_device_time + b.2;
                    start_a
                        .partial_cmp(&start_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut result_points = Vec::new();
                for (session_id, meta, offset) in sorted_sessions {
                    let session_start_unified = meta.first_device_time + offset;
                    let session_end_unified = meta.last_device_time + offset;

                    if session_end_unified < start_time || session_start_unified > end_time {
                        continue;
                    }

                    if let Some(buf_ref) = session_map.get(&session_id) {
                        let session_min_query = (start_time - offset).max(0.0);
                        let session_max_query = end_time - offset;
                        if session_min_query > session_max_query {
                            continue;
                        }

                        let min_bits = session_min_query.to_bits();
                        let max_bits = session_max_query.to_bits();

                        let map = buf_ref.data.read().unwrap();
                        for (t_bits, y) in map.range(min_bits..=max_bits) {
                            result_points.push(Point {
                                x: f64::from_bits(*t_bits) + offset,
                                y: *y,
                            });
                        }
                    }
                }
                result_points
            })
            .collect()
    }

    fn run_consumer(inner: Arc<Inner>, rx: Receiver<CaptureCommand>) {
        let self_instance = CaptureState {
            inner: inner.clone(),
        };
        while let Ok(command) = rx.recv() {
            match command {
                CaptureCommand::InsertBatch {
                    key,
                    points,
                    session_id,
                    instant,
                } => {
                    if !inner.active.contains_key(&key) {
                        continue;
                    }

                    let stream_key = key.stream_key();
                    let stream_state = inner.streams.entry(stream_key).or_default();

                    match stream_state.session_meta.entry(session_id) {
                        Entry::Occupied(mut e) => {
                            let meta = e.get_mut();
                            meta.last_instant = instant;
                            meta.last_device_time =
                                points.last().map(|p| p.x).unwrap_or(meta.last_device_time);
                        }
                        Entry::Vacant(e) => {
                            *stream_state.offsets_cache.lock().unwrap() = None;
                            let first_x = points.first().map(|p| p.x).unwrap_or_default();
                            let last_x = points.last().map(|p| p.x).unwrap_or(first_x);
                            e.insert(SessionMeta {
                                first_instant: instant,
                                last_instant: instant,
                                first_device_time: first_x,
                                last_device_time: last_x,
                            });
                        }
                    }

                    let session_map = inner.buffers.entry(key.clone()).or_default();
                    let rate =
                        stream_state.effective_sampling_rate.max(Self::DEFAULT_SAMPLING_RATE);
                    let cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);

                    if let Some(mut buf_ref) = session_map.get_mut(&session_id) {
                        buf_ref.value_mut().set_capacity(cap);
                        buf_ref.value_mut().push_many(&points);
                    } else {
                        let buf = Buffer::new(cap);
                        buf.push_many(&points);
                        session_map.insert(session_id, buf);
                    }

                    // This is the new distributor logic.
                    if let Some(subscribers) = inner.subscribers.get(&key) {
                        if subscribers.is_empty() {
                            continue;
                        }
                        let t_min = points.first().map(|p| p.x).unwrap_or(0.0);
                        let t_max = points.last().map(|p| p.x).unwrap_or(0.0);
                        let batch = Arc::new(BatchedData {
                            key: key.clone(),
                            session_id,
                            points: Arc::new(points),
                            t_min,
                            t_max,
                        });
                        // Fan out the batch to all subscribers for this key.
                        for (_, tx) in subscribers.iter() {
                            let _ = tx.try_send(batch.clone());
                        }
                    }
                }
                CaptureCommand::UpdateSampleRate { key, rate } => {
                    let stream_key = key.stream_key();
                    let mut stream_state = inner.streams.entry(stream_key.clone()).or_default();

                    if (stream_state.effective_sampling_rate - rate).abs() < 1e-9 {
                        continue;
                    }

                    stream_state.effective_sampling_rate = rate;
                    let new_cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);

                    inner
                        .active
                        .iter()
                        .filter(|r| r.key().stream_key() == stream_key)
                        .for_each(|r| {
                            let col_key = r.key();
                            if let Some(mut column_buffers) = inner.buffers.get_mut(col_key) {
                                for mut session_buffer in column_buffers.value_mut().iter_mut() {
                                    session_buffer.value_mut().set_capacity(new_cap);
                                }
                            }
                        });
                }
                CaptureCommand::SetActiveColumns {
                    port_url,
                    keys_for_port,
                } => {
                    inner.active.retain(|key, _| key.port_url != port_url);
                    for key in keys_for_port {
                        inner.active.insert(key, ());
                    }
                    println!(
                        "[Capture] Set {} active columns for port {}",
                        inner.active.len(),
                        port_url
                    );
                }
                CaptureCommand::CreateSnapshot {
                    plot_id,
                    keys,
                    start_time,
                    end_time,
                } => {
                    let raw_data_vecs =
                        self_instance.get_data_across_sessions_for_keys(&keys, start_time, end_time);

                    let mut individual_plot_data = Vec::with_capacity(keys.len());
                    for points in raw_data_vecs {
                        individual_plot_data.push(PlotData {
                            timestamps: points.iter().map(|p| p.x).collect(),
                            series_data: vec![points.iter().map(|p| p.y).collect()],
                        });
                    }
                    let snapshot_data = crate::util::k_way_merge_plot_data(individual_plot_data);

                    if !snapshot_data.is_empty() {
                        println!("[Capture] Created snapshot for plot {}", plot_id);
                        inner.paused_snapshots.insert(plot_id, snapshot_data);
                    }
                }
                CaptureCommand::ClearSnapshot { plot_id } => {
                    if inner.paused_snapshots.remove(&plot_id).is_some() {
                        println!("[Capture] Cleared snapshot for plot {}", plot_id);
                    }
                }
                CaptureCommand::Subscribe { key, id, tx } => {
                    inner.subscribers.entry(key).or_default().push((id, tx));
                }
                CaptureCommand::Unsubscribe { key, id } => {
                    if let Some(mut v) = inner.subscribers.get_mut(&key) {
                        v.retain(|(sid, _)| *sid != id);
                    }
                }
            }
        }
    }

    fn _get_or_compute_offsets_for_stream(
        &self,
        stream_key: &DataColumnId,
    ) -> Option<Arc<HashMap<SessionId, TimeOffset>>> {
        let stream_state = self.inner.streams.get(stream_key)?;

        {
            let cache = stream_state.offsets_cache.lock().unwrap();
            if let Some(offsets) = &*cache {
                if offsets.len() == stream_state.session_meta.len() {
                    return Some(offsets.clone());
                }
            }
        }

        let mut sorted_sessions: Vec<_> = stream_state
            .session_meta
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect();
        if sorted_sessions.is_empty() {
            return None;
        }

        sorted_sessions.sort_by_key(|(_id, meta)| meta.first_instant);

        let mut new_offsets = HashMap::new();
        let mut last_session_unified_end_time: UnifiedTime = 0.0;
        let mut last_meta: Option<SessionMeta> = None;

        for (session_id, current_meta) in sorted_sessions {
            let new_offset = if let Some(prev_meta) = last_meta.as_ref() {
                let gap = current_meta
                    .first_instant
                    .duration_since(prev_meta.last_instant);
                last_session_unified_end_time + gap.as_secs_f64() - current_meta.first_device_time
            } else {
                0.0
            };

            new_offsets.insert(session_id, new_offset);
            last_session_unified_end_time = current_meta.last_device_time + new_offset;
            last_meta = Some(current_meta);
        }

        let final_offsets_arc = Arc::new(new_offsets);

        {
            let mut cache = stream_state.offsets_cache.lock().unwrap();
            *cache = Some(final_offsets_arc.clone());
        }

        Some(final_offsets_arc)
    }

    pub fn get_effective_sampling_rate(&self, key: &DataColumnId) -> Option<f64> {
        self.inner
            .streams
            .get(&key.stream_key())
            .map(|stream_state_entry| stream_state_entry.value().effective_sampling_rate)
    }
}