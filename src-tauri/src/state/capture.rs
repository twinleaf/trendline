use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Instant};
use crate::shared::{DataColumnId, Point};
use crossbeam::channel::{unbounded, Receiver, Sender};
use std::thread;
use rayon::prelude::*;

pub type SessionId = u32;
pub type DeviceTime = f64;
pub type UnifiedTime = f64;
pub type TimeOffset = f64;

pub struct Buffer {
    pub data: BTreeMap<u64, f64>,
    pub cap: usize,
}

impl Buffer {
    fn new(cap: usize) -> Self {
        Self {
            data: BTreeMap::new(),
            cap,
        }
    }

    fn push(&mut self, p: Point) {
        self.data.insert(p.x.to_bits(), p.y);

        while self.data.len() > self.cap {
            if let Some(oldest_key) = self.data.keys().next().copied() {
                self.data.remove(&oldest_key);
            }
        }
    }

     pub fn set_capacity(&mut self, new_cap: usize) {
        if self.cap != new_cap {
            println!(
                "[Buffer] Capacity changed from {} to {}. Current size: {}",
                self.cap,
                new_cap,
                self.data.len()
            );
            self.cap = new_cap;
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

pub struct Inner {
    pub buffers: DashMap<DataColumnId, DashMap<SessionId, Buffer>>,
    pub streams: DashMap<DataColumnId, StreamState>,
    pub active: DashMap<DataColumnId, ()>,
    pub command_tx: Sender<CaptureCommand>,
}
#[derive(Debug)]
pub enum CaptureCommand {
    Insert {
        key: DataColumnId,
        data: Point,
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
    }
}

#[derive(Clone)]
pub struct CaptureState {
    pub inner: Arc<Inner>,
}

impl CaptureState {
    const BUFFER_WINDOW_SECONDS: f64 = 180.0;
    const DEFAULT_SAMPLING_RATE: f64 = 1000.0;

    pub fn new() -> Self {
        let (command_tx, command_rx) = unbounded();
        let inner = Arc::new(Inner {
            buffers: DashMap::new(),
            streams: DashMap::new(),
            active: DashMap::new(),
            command_tx,
        });

        let consumer_inner = inner.clone();
        thread::Builder::new()
            .name("capture-consumer".into())
            .spawn(move || Self::run_consumer(consumer_inner, command_rx))
            .expect("Failed to spawn CaptureState thread.");

        Self { inner }
    }
    
    pub fn get_latest_unified_timestamp(&self, keys: &[DataColumnId]) -> Option<UnifiedTime> {
        let unique_stream_keys: std::collections::HashSet<_> = keys.iter().map(|k| k.stream_key()).collect();

        let all_offsets = unique_stream_keys.iter()
            .filter_map(|stream_key| {
                self._get_or_compute_offsets_for_stream(stream_key)
                    .map(|offsets| (stream_key.clone(), offsets))
            })
            .collect::<HashMap<_, _>>();

        keys.iter()
            .filter_map(|key| {
                let offsets = all_offsets.get(&key.stream_key()).or_else(|| {
                    None
                })?;
                
                let (latest_session_id, max_offset) = offsets
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .or_else(|| {
                            None
                        })?;
                
                let binding = self.inner.buffers.get(key).or_else(|| {
                    None
                })?;

                let buffer = binding.get(latest_session_id).or_else(|| {
                    None
                })?;
                
                let (last_raw_ts_bits, _) = buffer.data.iter().next_back().or_else(|| {
                    None
                })?;

                let last_raw_ts = DeviceTime::from_bits(*last_raw_ts_bits);
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
        let unique_stream_keys: std::collections::HashSet<_> = keys.iter().map(|k| k.stream_key()).collect();
        let all_offsets = unique_stream_keys.iter()
            .filter_map(|stream_key| {
                self._get_or_compute_offsets_for_stream(stream_key)
                    .map(|offsets| (stream_key.clone(), offsets))
            })
            .collect::<HashMap<_, _>>();

        keys.par_iter()
            .map(|key| {
                let Some(offsets) = all_offsets.get(&key.stream_key()) else { return vec![]; };
                let Some(stream_state) = self.inner.streams.get(&key.stream_key()) else { return vec![]; };
                let Some(session_map) = self.inner.buffers.get(key) else { return vec![]; };
    
                let mut sorted_sessions: Vec<_> = stream_state.session_meta
                    .iter()
                    .filter_map(|entry| {
                        let session_id = *entry.key();
                        let meta = entry.value();
                        offsets.get(&session_id).map(|offset| (session_id, meta.clone(), *offset))
                    })
                    .collect();

                sorted_sessions.sort_by(|a, b| {
                    let start_a = a.1.first_device_time + a.2;
                    let start_b = b.1.first_device_time + b.2;
                    start_a.partial_cmp(&start_b).unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut result_points = Vec::new();
                for (session_id, meta, offset) in sorted_sessions {
                    let session_start_unified = meta.first_device_time + offset;
                    let session_end_unified = meta.last_device_time + offset;

                    if session_end_unified < start_time || session_start_unified > end_time {
                        continue;
                    }

                    if let Some(buffer) = session_map.get(&session_id) {
                        let session_min_query = (start_time - offset).max(0.0);
                        let session_max_query = end_time - offset;
                        if session_min_query > session_max_query { continue; }

                        let points_iter = buffer.data
                            .range(session_min_query.to_bits()..=session_max_query.to_bits())
                            .map(|(t_bits, y)| Point {
                                x: DeviceTime::from_bits(*t_bits) + offset,
                                y: *y,
                            });
                        result_points.extend(points_iter);
                    }
                }
                result_points
            })
            .collect()
    }

    fn run_consumer(inner: Arc<Inner>, rx: Receiver<CaptureCommand>) {
        while let Ok(command) = rx.recv() {
            match command {
                CaptureCommand::Insert { key, data, session_id, instant } => {
                    if !inner.active.contains_key(&key) { continue; }

                    let stream_key = key.stream_key();
                    let stream_state = inner.streams.entry(stream_key).or_default();
                    match stream_state.session_meta.entry(session_id) {
                        Entry::Occupied(mut entry) => {
                            let meta = entry.get_mut();
                            meta.last_instant = instant;
                            meta.last_device_time = data.x;
                        }
                        Entry::Vacant(entry) => {
                            *stream_state.offsets_cache.lock().unwrap() = None;
                            entry.insert(SessionMeta {
                                first_instant: instant, last_instant: instant,
                                first_device_time: data.x, last_device_time: data.x,
                            });
                        }
                    }

                    let session_map = inner.buffers.entry(key.clone()).or_default();
                    let mut buffer = session_map.entry(session_id).or_insert_with(|| {
                        let rate = stream_state.effective_sampling_rate.max(Self::DEFAULT_SAMPLING_RATE);
                        let cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);
                        Buffer::new(cap)
                    });
                    buffer.push(data);
                }
                 CaptureCommand::UpdateSampleRate { key, rate } => {
                    let stream_key = key.stream_key();
                    let mut stream_state = inner.streams.entry(stream_key.clone()).or_default();

                    if (stream_state.effective_sampling_rate - rate).abs() < 1e-9 { continue; }
                    
                    stream_state.effective_sampling_rate = rate;
                    let new_cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);

                    inner.active.iter()
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
                CaptureCommand::SetActiveColumns { port_url, keys_for_port } => {
                    inner.active.retain(|key, _| key.port_url != port_url);
                    for key in keys_for_port {
                        inner.active.insert(key, ());
                    }
                    println!("[Capture] Set {} active columns for port {}", inner.active.len(), port_url);
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

        let mut sorted_sessions: Vec<_> = stream_state.session_meta.iter()
            .map(|entry| (*entry.key(), entry.value().clone())).collect();
        if sorted_sessions.is_empty() { return None; }
        
        sorted_sessions.sort_by_key(|(_id, meta)| meta.first_instant);
        
        let mut new_offsets = HashMap::new();
        let mut last_session_unified_end_time: UnifiedTime = 0.0;
        let mut last_meta: Option<SessionMeta> = None;

        for (session_id, current_meta) in sorted_sessions {
            let new_offset = if let Some(prev_meta) = last_meta.as_ref() {
                let gap = current_meta.first_instant.duration_since(prev_meta.last_instant);
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

    pub fn get_effective_sampling_rate(&self, stream_key: &DataColumnId) -> Option<f64> {
        self.inner.streams.get(&stream_key.stream_key())
            .map(|stream_state_entry| stream_state_entry.value().effective_sampling_rate)
    }
}
