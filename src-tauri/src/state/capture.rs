use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Instant};
use crate::state::decimation::{lerp, fpcs, min_max_bucket};
use crate::shared::{DataColumnId, Point, StatisticSet};
use crossbeam::channel::{unbounded, Receiver, Sender};
use std::thread;

use crate::shared::{ PlotData, DecimationMethod };

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

#[derive(Clone, Debug)]
pub struct PersistentStat {
    pub count: u64,
    pub mean: f64,
    m2: f64,
    sum_of_squares: f64,
    pub min: f64,
    pub max: f64,
}

impl PersistentStat {
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

    pub fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }

    pub fn stdev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn rms(&self) -> f64 {
        if self.count == 0 { 0.0 } else { (self.sum_of_squares / self.count as f64).sqrt() }
    }
}

impl From<&PersistentStat> for StatisticSet {
    fn from(stat: &PersistentStat) -> Self {
        Self {
            count: stat.count,
            mean: stat.mean,
            min: stat.min,
            max: stat.max,
            stdev: stat.stdev(),
            rms: stat.rms(),
        }
    }
}

// The shared data that needs to be accessed by multiple threads.
pub struct Inner {
    pub buffers: DashMap<DataColumnId, DashMap<SessionId, Buffer>>,
    pub session_meta: DashMap<DataColumnId, DashMap<SessionId, SessionMeta>>,
    pub offsets_cache: DashMap<DataColumnId, Arc<HashMap<SessionId, TimeOffset>>>,
    pub persistent_stats: DashMap<DataColumnId, Mutex<PersistentStat>>,
    pub active: DashMap<DataColumnId, ()>,
    pub effective_sampling_rates: DashMap<DataColumnId, f64>,
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
}

#[derive(Clone)]
pub struct CaptureState {
    pub inner: Arc<Inner>,
}

impl CaptureState {
    const BUFFER_WINDOW_SECONDS: f64 = 180.0;

    pub fn new() -> Self {
        let (command_tx, command_rx) = unbounded();
        let inner = Arc::new(Inner {
            buffers: DashMap::new(),
            session_meta: DashMap::new(),
            offsets_cache: DashMap::new(),
            active: DashMap::new(),
            effective_sampling_rates: DashMap::new(),
            persistent_stats: DashMap::new(),
            command_tx,
        });

        let consumer_inner = inner.clone();
        thread::Builder::new()
            .name("capture-consumer".into())
            .spawn(move || Self::run_consumer(consumer_inner, command_rx))
            .expect("Failed to spawn CaptureState thread.");

        Self { inner }
    }

    pub fn get_plot_data(
        &self,
        keys: &[DataColumnId],
        start_time: UnifiedTime,
        end_time: UnifiedTime,
        num_points: Option<usize>,
        decimation: Option<DecimationMethod>,
        ) -> PlotData {
            let stitched_series = self.get_data_across_sessions_for_keys(keys, start_time, end_time);
            self._process_and_merge_series(stitched_series, start_time, end_time, num_points, decimation)
        }

    pub fn get_latest_unified_timestamp(&self, keys: &[DataColumnId]) -> Option<UnifiedTime> {
        let all_offsets = self._get_or_compute_global_offsets(keys);

        keys.iter()
            .filter_map(|key| {
                let offsets = all_offsets.get(key)?;

                let (latest_session_id, max_offset) = offsets
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?;

                let binding = self.inner.buffers.get(key)?;
                let buffer = binding.get(latest_session_id)?;

                let (last_raw_ts_bits, _) = buffer.data.iter().next_back()?;
                
                let last_raw_ts = DeviceTime::from_bits(*last_raw_ts_bits);
                Some(last_raw_ts + max_offset)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
    
    pub fn get_interpolated_values_at_time(
        &self,
        keys: &[DataColumnId],
        time: UnifiedTime,
    ) -> Vec<Option<f64>> {
        let all_offsets = self._get_or_compute_global_offsets(keys);

        keys.iter()
            .map(|key| {
                let find_value = || -> Option<f64> {
                    let offsets = all_offsets.get(key)?;
                    let meta_map = self.inner.session_meta.get(&key.device_key())?;

                    for entry in meta_map.iter() {
                        let session_id = entry.key();
                        let meta = entry.value();
                        let offset = offsets.get(session_id).copied().unwrap_or(0.0);

                        let session_start_unified = meta.first_device_time + offset;
                        let session_end_unified = meta.last_device_time + offset;

                        if time >= session_start_unified && time <= session_end_unified {
                            let raw_time_needed = time - offset;
                            let session_map = self.inner.buffers.get(key)?;
                            let buffer = session_map.get(session_id)?;

                            let p1_opt = buffer.data.range(..=raw_time_needed.to_bits()).next_back();
                            let p2_opt = buffer.data.range(raw_time_needed.to_bits()..).next();
                            
                            return match (p1_opt, p2_opt) {
                                (Some((t1_bits, y1)), Some((t2_bits, y2))) => {
                                    let t1 = DeviceTime::from_bits(*t1_bits);
                                    if (t1 - raw_time_needed).abs() < 1e-9 {
                                        return Some(*y1);
                                    }
                                    
                                    let p1 = Point::new(t1, *y1);
                                    let p2 = Point::new(DeviceTime::from_bits(*t2_bits), *y2);
                                    Some(lerp(&p1, &p2, raw_time_needed))
                                }
                                (Some((_, y1)), None) => Some(*y1),
                                (None, Some((_, y2))) => Some(*y2),
                                (None, None) => None,
                            };
                        }
                    }
                    None
                };
                find_value()
            })
            .collect()
    }

    pub fn get_data_across_sessions_for_keys(
        &self,
        keys: &[DataColumnId],
        start_time: UnifiedTime,
        end_time: UnifiedTime,
    ) -> Vec<Vec<Point>> {
        let all_offsets = self._get_or_compute_global_offsets(keys);

        keys.iter()
            .map(|key| {
                let Some(offsets) = all_offsets.get(key) else { return vec![]; };
                let Some(meta_map) = self.inner.session_meta.get(&key.device_key()) else { return vec![]; };
                let Some(session_map) = self.inner.buffers.get(key) else { return vec![]; };

                let mut sorted_sessions: Vec<_> = meta_map
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
                        let session_min_query = start_time - offset;
                        let session_max_query = end_time - offset;

                        let clamped_min_query = session_min_query.max(0.0);

                        if clamped_min_query > session_max_query {
                            continue;
                        }

                        let points_iter = buffer
                            .data
                            .range(clamped_min_query.to_bits()..=session_max_query.to_bits())
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

                    inner.persistent_stats
                        .entry(key.clone())
                        .or_insert_with(|| Mutex::new(PersistentStat::new(data.y)))
                        .lock()
                        .unwrap()
                        .update(data.y);

                    let device_key = key.device_key();
                    let meta_map = inner.session_meta.entry(device_key).or_default();
                    match meta_map.entry(session_id) {
                        Entry::Occupied(mut entry) => {
                            let meta = entry.get_mut();
                            meta.last_instant = instant;
                            meta.last_device_time = data.x;
                        }
                        Entry::Vacant(entry) => {
                            println!(
                                "[Capture] New session detected for device {}:{}, Session ID: {}",
                                key.port_url, key.device_route, session_id
                            );
                            entry.insert(SessionMeta {
                                first_instant: instant,
                                last_instant: instant,
                                first_device_time: data.x,
                                last_device_time: data.x,
                            });
                        }
                    }

                    let session_map = inner.buffers.entry(key.clone()).or_default();
                    let mut buffer = session_map.entry(session_id).or_insert_with(|| {
                        let stream_key = key.stream_key();
                        let rate = inner.effective_sampling_rates.get(&stream_key).map_or(1000.0, |r| *r.value());
                        let cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);
                        Buffer::new(cap)
                    });
                    buffer.push(data);
                }
                CaptureCommand::UpdateSampleRate { key, rate } => {
                    let stream_key = key.stream_key();
                    if let Some(r) = inner.effective_sampling_rates.get(&stream_key) {
                        if (r.value() - rate).abs() < 1e-9 { continue; }
                    }
                    inner.effective_sampling_rates.insert(stream_key.clone(), rate);
                    let new_cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);

                    println!(
                        "[Capture] Updating buffer capacity for stream {}:{}/{} to {}. (New Rate: {} Hz)",
                        stream_key.port_url, stream_key.device_route, stream_key.stream_id, new_cap, rate
                    );

                    inner.active
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
            }
        }
    }

    fn _get_or_compute_global_offsets(
        &self,
        keys: &[DataColumnId],
    ) -> HashMap<DataColumnId, Arc<HashMap<SessionId, TimeOffset>>> {
        let mut device_level_offsets = HashMap::new();
        let unique_device_keys: std::collections::HashSet<_> = keys.iter().map(|k| k.device_key()).collect();

        for device_key in unique_device_keys {
            let Some(session_meta_map) = self.inner.session_meta.get(&device_key) else { continue; };
            if session_meta_map.is_empty() {
                continue;
            };

            let mut recompute = false; 

            if let Some(cached_arc) = self.inner.offsets_cache.get(&device_key) {
                if cached_arc.len() == session_meta_map.len() {
                    device_level_offsets.insert(device_key.clone(), cached_arc.clone());
                } else {
                    recompute = true;
                }
            } else {
                recompute = true;
            }

            if recompute {
                let mut new_offsets = HashMap::new();

                let mut sorted_sessions: Vec<_> = session_meta_map.iter().map(|entry| (*entry.key(), entry.value().clone())).collect();
                sorted_sessions.sort_by_key(|(_id, meta)| meta.first_instant);

                println!("[Offsets] Calculating for device {}:{} with sorted sessions:", device_key.port_url, device_key.device_route);
                for (id, meta) in &sorted_sessions {
                    println!("  - Session ID: {}, First Device Time: {:.4}, First Instant: {:?}", id, meta.first_device_time, meta.first_instant);
                }

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
                self.inner.offsets_cache.insert(device_key.clone(), final_offsets_arc.clone());
                device_level_offsets.insert(device_key, final_offsets_arc);
            }
        }

        keys.iter()
            .filter_map(|col_key| {
                device_level_offsets
                    .get(&col_key.device_key())
                    .map(|offsets| (col_key.clone(), offsets.clone()))
            })
            .collect()
    }

    fn _process_and_merge_series(
        &self,
        mut continuous_series: Vec<Vec<Point>>,
        min_time: f64,
        max_time: f64,
        num_points: Option<usize>,
        decimation: Option<DecimationMethod>,
    ) -> PlotData {
        if let (Some(n_points), Some(method)) = (num_points, decimation) {
            if method == DecimationMethod::MinMax && n_points > 0 {
                let mut decimated_series_data = Vec::with_capacity(continuous_series.len());
                let mut final_timestamps = Vec::new();

                for (i, series) in continuous_series.iter_mut().enumerate() {
                    if series.len() > n_points {
                        *series = min_max_bucket(series, n_points, min_time, max_time);
                    }
                    
                    if i == 0 {
                        final_timestamps = series.iter().map(|p| p.x).collect();
                    }
                    decimated_series_data.push(series.iter().map(|p| p.y).collect());
                }
                return PlotData {
                    timestamps: final_timestamps,
                    series_data: decimated_series_data,
                };
            }
            
            if !matches!(method, DecimationMethod::None) {
                for series in &mut continuous_series {
                    if series.len() > n_points && n_points > 0 {
                        *series = match method {
                            DecimationMethod::Fpcs => {
                                let ratio = series.len() / n_points;
                                if ratio > 1 { fpcs(series, ratio) } else { series.clone() }
                            },
                            DecimationMethod::MinMax => series.clone(), 
                            DecimationMethod::None => unreachable!(),
                        };
                    }
                }
            }
        }

        // K-Way Merge Logic
        let mut series_iters: Vec<_> = continuous_series.iter().map(|s| s.iter().peekable()).collect();
        if series_iters.is_empty() { return PlotData::empty(); }

        let num_series = series_iters.len();
        let mut plot_data = PlotData::with_series_capacity(num_series);
        let mut last_points: Vec<Option<Point>> = vec![None; num_series];
        loop {
            let next_ts = series_iters.iter_mut().filter_map(|it| it.peek().map(|p| p.x))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(ts) = next_ts {
                plot_data.timestamps.push(ts);
                for i in 0..num_series {
                    let iter = &mut series_iters[i];
                    let mut y_val = f64::NAN;
                    if iter.peek().map_or(false, |p| (p.x - ts).abs() < 1e-9) {
                        let p_next = iter.next().unwrap();
                        last_points[i] = Some(*p_next);
                        y_val = p_next.y;
                    } else if let (Some(p_next_real), Some(p_last)) = (iter.peek(), last_points[i]) {
                        y_val = lerp(&p_last, p_next_real, ts);
                    } else if let Some(p_last) = last_points[i] {
                        y_val = p_last.y;
                    } else if let Some(p_next_real) = iter.peek() {
                        y_val = p_next_real.y;
                    }
                    plot_data.series_data[i].push(y_val);
                }
            } else { break; }
        }
        plot_data
    }

    pub fn start_capture(&self, key: &DataColumnId) {
        self.inner.active.insert(key.clone(), ());
    }

    pub fn stop_capture(&self, key: &DataColumnId) {
        self.inner.active.remove(key);
    }

    pub fn set_active_columns_for_port(&self, port_url: &str, keys_for_port: Vec<DataColumnId>) {
        println!("[{}] Built and activating {} total data columns", port_url, keys_for_port.len());
        self.inner.active.retain(|key, _value| key.port_url != port_url);

        for key in keys_for_port {
            if key.port_url == port_url {
                self.start_capture(&key);
            }
        }
    }

    pub fn update_effective_sampling_rate(&self, key: &DataColumnId, rate: f64) {
        self.inner.effective_sampling_rates.insert(key.stream_key(), rate);
    }

    pub fn get_effective_sampling_rate(&self, key: &DataColumnId) -> Option<f64> {
        self.inner.effective_sampling_rates.get(&key.stream_key()).map(|r| *r.value())
    }


}