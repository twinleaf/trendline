use dashmap::DashMap;
use serde::Deserialize;
use ts_rs::TS;
use dashmap::mapref::entry::Entry;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use twinleaf::tio::proto::DeviceRoute;
use crate::state::decimation::{lerp, fpcs, min_max_bucket};
use crate::util;
use crossbeam::channel::{unbounded, Sender, Receiver};
use std::thread;

use crate::shared::{ PlotData, DecimationMethod };

// The unique identifier for a single column of data from a specific device.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct DataColumnId {
    pub port_url: String,
    #[serde(with = "util")]
    #[ts(type = "string")]
    pub device_route: DeviceRoute,
    pub stream_id: u8,
    pub column_index: usize,
}

impl DataColumnId {
    pub fn stream_key(&self) -> Self {
        Self { column_index: 0, ..self.clone() }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

// A circular buffer for a single time series.
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
        self.cap = new_cap;
    }
}

#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub first_wall_time: SystemTime,
    pub last_wall_time: SystemTime,
    pub first_device_time: f64,
    pub last_device_time: f64,
}

// The shared data that needs to be accessed by multiple threads.
pub struct Inner {
    pub buffers: DashMap<DataColumnId, DashMap<u32, Buffer>>,
    pub session_meta: DashMap<DataColumnId, DashMap<u32, SessionMeta>>,
    pub active: DashMap<DataColumnId, ()>,
    pub effective_sampling_rates: DashMap<DataColumnId, f64>,
    pub initial_buffer_cap: usize,
    pub command_tx: Sender<CaptureCommand>,
}

#[derive(Debug)]
pub enum CaptureCommand {
    Insert {
        key: DataColumnId, 
        data: Point,
        session_id: u32,
        wall_time: SystemTime,
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
        let (command_tx, command_rx): (Sender<CaptureCommand>, Receiver<CaptureCommand>) = unbounded();

        let inner = Arc::new(Inner {
                buffers: DashMap::new(),
                session_meta: DashMap::new(),
                active: DashMap::new(),
                effective_sampling_rates: DashMap::new(),
                initial_buffer_cap: 200_000, // e.g., 1000 samples/sec * 200 seconds, 1600 samples/sec * 125 seconds
                command_tx,
             });

        let consumer_inner = inner.clone();
        thread::Builder::new().name("capture-consumer".into()).spawn(move || {
            Self::run_consumer(consumer_inner, command_rx);
        }).expect("Failed to spawn CaptureState thread.");

        Self { inner }
    }

    pub fn get_data_in_range(
        &self,
        keys: &[DataColumnId],
        min_time: f64,
        max_time: f64,
        num_points: Option<usize>,
        decimation: Option<DecimationMethod>,
    ) -> PlotData {
        if keys.is_empty() || min_time >= max_time {
            return PlotData::empty();
        }
        
        let stitched_series = self.get_stitched_series_in_range(keys, min_time, max_time);

        self.process_and_merge_series(stitched_series, min_time, max_time, num_points, decimation)
    }

    pub fn get_latest_timestamp_for_keys(&self, keys: &[DataColumnId]) -> Option<f64> {
        keys.iter()
            .filter_map(|key| {
                let session_map = self.inner.buffers.get(key)?;
                let latest_session_id = session_map.iter().map(|entry| *entry.key()).max()?;
                let buffer = session_map.get(&latest_session_id)?;

                let (last_ts_bits, _) = buffer.data.iter().next_back()?;
                Some(f64::from_bits(*last_ts_bits))
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn get_interpolated_values_at_time(&self, keys: &[DataColumnId], time: f64) -> Vec<Option<f64>> {
        keys.iter().map(|key| {
            let window_for_search = 10.0;
            let stitched_points = self.get_stitched_series_in_range(
                &[key.clone()],
                time - window_for_search,
                time + window_for_search,
            );
            
            if let Some(points) = stitched_points.first() {
                if points.is_empty() {
                    return None;
                }
                

                let p2_index_opt = points.iter().position(|p| p.x >= time);

                match p2_index_opt {
                    Some(p2_index) => {
                        if (points[p2_index].x - time).abs() < 1e-9 {
                            return Some(points[p2_index].y);
                        }
                        
                        if p2_index > 0 {
                            let p1 = points[p2_index - 1];
                            let p2 = points[p2_index];
                            Some(lerp(&p1, &p2, time))
                        } else {
                            Some(points[p2_index].y)
                        }
                    }
                    None => {
                        points.last().map(|p| p.y)
                    }
                }
            } else {
                None
            }
        }).collect()
    }

    fn run_consumer(inner: Arc<Inner>, rx: Receiver<CaptureCommand>) {
        while let Ok(command) = rx.recv() {
            match command {
                CaptureCommand::Insert { key, data: p, session_id, wall_time } => {
                    if !inner.active.contains_key(&key) {
                        continue;
                    }

                    let meta_map = inner.session_meta.entry(key.clone()).or_default();
                    match meta_map.entry(session_id) {
                        Entry::Occupied(mut entry) => {
                            let meta = entry.get_mut();
                            meta.last_wall_time = wall_time;
                            meta.last_device_time = p.x;
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(SessionMeta {
                                first_wall_time: wall_time,
                                last_wall_time: wall_time,
                                first_device_time: p.x,
                                last_device_time: p.x,
                            });
                        }
                    }

                    let session_map = inner.buffers.entry(key.clone()).or_default();
                    let mut buffer = session_map.entry(session_id).or_insert_with(|| {
                        let stream_key = key.stream_key();
                        let rate = inner
                            .effective_sampling_rates
                            .get(&stream_key)
                            .map_or(1000.0, |r| *r.value());
                        let initial_cap =
                            ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);
                        Buffer::new(initial_cap)
                    });
                    buffer.push(p);
                }
                CaptureCommand::UpdateSampleRate { key, rate } => {
                    let stream_key = key.stream_key();
                    if let Some(existing_rate) = inner.effective_sampling_rates.get(&stream_key) {
                        if (existing_rate.value() - rate).abs() < 1e-9 {
                            continue;
                        }
                    }
                    println!(
                        "[Capture] Updating sample rate for stream at route '{}' to {} Hz",
                        key.device_route, rate
                    );
                    inner.effective_sampling_rates.insert(stream_key.clone(), rate);
                    let new_cap = ((rate * Self::BUFFER_WINDOW_SECONDS) as usize).max(100);

                    if let Some(session_map) = inner.buffers.get(&stream_key) {
                        for mut buffer in session_map.iter_mut() {
                            println!(
                                "[Capture] Resizing buffer for {:?} (Session {}) to {} points",
                                key, buffer.key(), new_cap
                            );
                            buffer.set_capacity(new_cap);
                        }
                    }
                }
            }
        }
    }

    fn collect_points_for_key(
        &self,
        key: &DataColumnId,
        window_seconds: f64,
    ) -> Vec<Point> {
        let Some(session_map) = self.inner.buffers.get(key) else { return vec![]; };
        let Some(meta_map) = self.inner.session_meta.get(key) else { return vec![]; };

        let mut session_ids: Vec<_> = session_map.iter().map(|r| *r.key()).collect();
        session_ids.sort_unstable_by(|a, b| b.cmp(a));

        let mut collected_points: Vec<Point> = Vec::new();
        let mut time_to_gather = window_seconds;
        let mut time_offset = 0.0;

        for (i, &session_id) in session_ids.iter().enumerate() {
            if time_to_gather <= 0.0 { break; }

            let Some(buffer) = session_map.get(&session_id) else { continue; };
            let Some(meta) = meta_map.get(&session_id) else { continue; };
            if buffer.data.is_empty() { continue; }
            
            let session_points: Vec<Point> = buffer.data.iter()
                .map(|(&t_bits, &y)| Point::new(f64::from_bits(t_bits) + time_offset, y))
                .collect();

            let session_adjusted_min_ts = session_points.first().unwrap().x;
            let session_adjusted_max_ts = session_points.last().unwrap().x;
            let session_duration = session_adjusted_max_ts - session_adjusted_min_ts;
            
            let points_to_prepend = if session_duration >= time_to_gather {
                let required_start_time = session_adjusted_max_ts - time_to_gather;
                let start_idx = session_points.iter().position(|p| p.x >= required_start_time).unwrap_or(0);
                time_to_gather = 0.0;
                session_points[start_idx..].to_vec()
            } else {
                time_to_gather -= session_duration;
                session_points
            };

            collected_points.splice(0..0, points_to_prepend);
            
            if i + 1 < session_ids.len() {
            let next_session_id = session_ids[i + 1];
            if let Some(next_meta) = meta_map.get(&next_session_id) {
                let wall_clock_gap = meta
                    .first_wall_time
                    .duration_since(next_meta.last_wall_time)
                    .unwrap_or(Duration::ZERO);

                let unified_current_session_start = meta.first_device_time + time_offset;
                
                let next_session_total_offset = unified_current_session_start
                    - wall_clock_gap.as_secs_f64()
                    - next_meta.last_device_time;

                time_offset = next_session_total_offset;
            }
        }
    }
    collected_points
}

    fn get_stitched_series_in_range(
        &self,
        keys: &[DataColumnId],
        min_time: f64,
        max_time: f64,
    ) -> Vec<Vec<Point>> {
        keys.iter()
            .map(|key| {
                let window_seconds = (max_time - min_time) + Self::BUFFER_WINDOW_SECONDS;
                
                let all_stitched_points = self.collect_points_for_key(key, window_seconds);

                all_stitched_points
                    .into_iter()
                    .filter(|p| p.x >= min_time && p.x <= max_time)
                    .collect::<Vec<Point>>()
            })
            .collect()
    }

    fn process_and_merge_series(
        &self,
        mut aligned_series: Vec<Vec<Point>>,
        min_time: f64,
        max_time: f64,
        num_points: Option<usize>,
        decimation: Option<DecimationMethod>,
    ) -> PlotData {
        if let (Some(n_points), Some(method)) = (num_points, decimation) {
            if method == DecimationMethod::MinMax && n_points > 0 {
                let mut decimated_series_data = Vec::with_capacity(aligned_series.len());
                let mut final_timestamps = Vec::new();

                for (i, series) in aligned_series.iter_mut().enumerate() {
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
                for series in &mut aligned_series {
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
        let mut series_iters: Vec<_> = aligned_series.iter().map(|s| s.iter().peekable()).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::PlotData;

    fn test_merge(series_data: Vec<Vec<Point>>) -> PlotData {
        let mut series_iters: Vec<_> = series_data
            .iter()
            .map(|series| series.iter().peekable())
            .collect();
        
        let mut plot_data = PlotData::with_series_capacity(series_data.len());

        loop {
            let next_ts = series_iters
                .iter_mut()
                .filter_map(|it| it.peek().map(|p| p.x))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            if let Some(ts) = next_ts {
                plot_data.timestamps.push(ts);
                for i in 0..series_iters.len() {
                    let y_val = if let Some(p) = series_iters[i].peek() {
                        if p.x == ts {
                            let y = p.y;
                            series_iters[i].next();
                            y
                        } else {
                            f64::NAN
                        }
                    } else {
                        f64::NAN
                    };
                    plot_data.series_data[i].push(y_val);
                }
            } else {
                break;
            }
        }
        plot_data
    }

    #[test]
    fn test_k_way_merge_inserts_nan() {
        // Two series, downsampled independently. They have different timestamps.
        let series1 = vec![Point { x: 1.0, y: 10.0 }, Point { x: 3.0, y: 12.0 }];
        let series2 = vec![Point { x: 2.0, y: 100.0 }, Point { x: 4.0, y: 120.0 }];

        let result = test_merge(vec![series1, series2]);

        // Expected unified timestamps: [1.0, 2.0, 3.0, 4.0]
        assert_eq!(result.timestamps, vec![1.0, 2.0, 3.0, 4.0]);

        // Expected data for series 1: [10.0, NaN, 12.0, NaN]
        assert_eq!(result.series_data[0][0], 10.0);
        assert!(result.series_data[0][1].is_nan());
        assert_eq!(result.series_data[0][2], 12.0);
        assert!(result.series_data[0][3].is_nan());

        // Expected data for series 2: [NaN, 100.0, NaN, 120.0]
        assert!(result.series_data[1][0].is_nan());
        assert_eq!(result.series_data[1][1], 100.0);
        assert!(result.series_data[1][2].is_nan());
        assert_eq!(result.series_data[1][3], 120.0);
    }
}