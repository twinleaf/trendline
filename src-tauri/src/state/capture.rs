use dashmap::DashMap;
use serde::Deserialize;
use ts_rs::TS;
use std::collections::BTreeMap;
use std::sync::Arc;
use twinleaf::tio::proto::DeviceRoute;
use crate::state::decimation::{fpcs};
use crate::util;

use crate::shared::PlotData;

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

        if self.data.len() > self.cap {
            if let Some(oldest_key) = self.data.keys().next().copied() {
                self.data.remove(&oldest_key);
            }
        }
    }
}

// The shared data that needs to be accessed by multiple threads.
pub struct Inner {
    pub buffers: DashMap<DataColumnId, Buffer>,
    pub active: DashMap<DataColumnId, ()>,
    pub offsets: DashMap<DataColumnId, f64>,
    pub default_cap: usize,
}

#[derive(Clone)]
pub struct CaptureState {
    pub inner: Arc<Inner>,
}

impl CaptureState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                buffers: DashMap::new(),
                active: DashMap::new(),
                offsets: DashMap::new(),
                default_cap: 120_000, // e.g., 1000 samples/sec * 120 seconds
            }),
        }
    }

    pub fn get_data_in_range(
        &self,
        keys: &[DataColumnId],
        min_time: f64,
        max_time: f64,
        num_points: Option<usize>,
    ) -> PlotData {
        if keys.is_empty() || min_time >= max_time {
            return PlotData::empty();
        }
        let mut aligned_series: Vec<Vec<Point>> = keys
        .iter()
        .map(|key| {
            let stream_key = DataColumnId { column_index: 0, ..key.clone() };
            let offset = self.inner.offsets.get(&stream_key).map_or(0.0, |off| *off.value());

            let min_key = (min_time - offset).to_bits();
            let max_key = (max_time - offset).to_bits();

            self.inner.buffers.get(key)
                .map(|buffer| {
                    buffer.data
                        .range(min_key..=max_key)
                        .map(|(&t_bits, &y)| {
                            let original_timestamp = f64::from_bits(t_bits);
                            Point::new(original_timestamp + offset, y)
                        })
                        .collect()
                })
                .unwrap_or_else(Vec::new)
        })
        .collect();
            
        if let Some(n_points) = num_points {
            for series in &mut aligned_series {
                if !series.is_empty() && n_points > 0 && series.len() > n_points {
                    let ratio = series.len() / n_points;
                    *series = fpcs(series, ratio);
                }
            }
        }

        let mut series_iters: Vec<_> = aligned_series
            .iter()
            .map(|series| series.iter().peekable())
            .collect();

        if series_iters.is_empty() {
            return PlotData::empty();
        }

        let num_series = series_iters.len();
        let mut plot_data = PlotData::with_series_capacity(num_series);

        loop {

            let next_ts = series_iters
                .iter_mut()
                .filter_map(|it| it.peek().map(|p| p.x))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            if let Some(ts) = next_ts {
                plot_data.timestamps.push(ts);

                for (i, iter) in series_iters.iter_mut().enumerate() {
                    let y_val = if let Some(p) = iter.peek() {
                        if p.x == ts {
                            let y = p.y;
                            iter.next();
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

    pub fn get_latest_timestamp_for_keys(&self, keys: &[DataColumnId]) -> Option<f64> {
        let mut max_ts_bits = 0u64;
        let mut found_any = false;

        for key in keys {
            if let Some(buffer) = self.inner.buffers.get(key) {
                if let Some((&ts_bits, _)) = buffer.data.iter().next_back() {
                    if ts_bits > max_ts_bits {
                        max_ts_bits = ts_bits;
                        found_any = true;
                    }
                }
            }
        }

        if !found_any {
            return None;
        }

        let stream_key = DataColumnId { column_index: 0, ..keys[0].clone() };
        let offset = self.inner.offsets.get(&stream_key).map_or(0.0, |o| *o.value());

        Some(f64::from_bits(max_ts_bits) + offset)
    }

    /// Inserts a data point for a given column, but only if that column is active.
    pub fn insert(&self, key: &DataColumnId, p: Point) {
        if !self.inner.active.contains_key(key) {
            return;
        }
        let stream_key = DataColumnId { column_index: 0, ..key.clone() };
        self.inner.offsets.entry(stream_key).or_insert_with(|| {
            println!(
                "[Capture] New stream activated (Port: {}, Device: {}, Stream: {}). Setting t=0 baseline from first timestamp {}.",
                key.port_url, key.device_route, key.stream_id, p.x
            );
            -p.x // The offset is the negative of the first point's timestamp.
        });

        // Store the point with its ORIGINAL, UNMODIFIED timestamp.
        let mut buffer = self
            .inner
            .buffers
            .entry(key.clone())
            .or_insert_with(|| Buffer::new(self.inner.default_cap));
        
        buffer.push(p);
    }

    pub fn start_capture(&self, key: &DataColumnId) {
        self.inner.active.insert(key.clone(), ());
    }

    pub fn stop_capture(&self, key: &DataColumnId) {
        self.inner.active.remove(key);
        self.inner.offsets.remove(key);
    }

    pub fn set_active_columns_for_port(&self, port_url: &str, keys_for_port: Vec<DataColumnId>) {
        self.inner.active.retain(|key, _value| key.port_url != port_url);

        for key in keys_for_port {
            if key.port_url == port_url {
                self.start_capture(&key);
            }
        }
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