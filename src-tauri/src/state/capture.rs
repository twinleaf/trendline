use dashmap::DashMap;
use serde::Deserialize;
use ts_rs::TS;
use std::collections::BTreeMap;
use std::sync::Arc;
use twinleaf::tio::proto::DeviceRoute;
use crate::state::decimation::{lerp, fpcs, min_max_bucket};
use crate::util;

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
    pub effective_sampling_rates: DashMap<DataColumnId, f64>,
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
                effective_sampling_rates: DashMap::new(),
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
        decimation: Option<DecimationMethod>,
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
            
        if let (Some(n_points), Some(method)) = (num_points, decimation) {
            if !matches!(method, DecimationMethod::None) {
                for series in &mut aligned_series {
                    if series.len() > n_points && n_points > 0 {
                        *series = match method {
                            DecimationMethod::Fpcs => {
                                let ratio = series.len() / n_points;
                                if ratio > 1 { fpcs(series, ratio) } else { series.clone() }
                            },
                            DecimationMethod::MinMax => min_max_bucket(series, n_points, min_time, max_time),
                            DecimationMethod::None => unreachable!(),
                        };
                    }
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
        let mut last_points: Vec<Option<Point>> = vec![None; series_iters.len()];

        loop {
            let next_ts = series_iters
                .iter_mut()
                .filter_map(|it| it.peek().map(|p| p.x))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            if let Some(ts) = next_ts {
                plot_data.timestamps.push(ts);

                for i in 0..series_iters.len() {
                    let iter = &mut series_iters[i];
                    let mut y_val = f64::NAN;

                    let next_point_matches = if let Some(p) = iter.peek() {
                        p.x == ts
                    } else {
                        false
                    };

                    if next_point_matches {
                        let p_next = iter.next().unwrap(); 
                        last_points[i] = Some(*p_next);
                        y_val = p_next.y;
                    } else {
                        if let (Some(p_last), Some(p_next_real)) = (last_points[i], iter.peek()) {
                            y_val = lerp(&p_last, p_next_real, ts);
                        }
                    }
                    plot_data.series_data[i].push(y_val);
                }
            } else {
                break;
            }
        }

        plot_data
    }

    pub fn get_latest_timestamp_for_keys(&self, keys: &[DataColumnId]) -> Option<f64> {
        keys.iter()
            .filter_map(|key| {
                let buffer = self.inner.buffers.get(key)?;
                let (last_raw_ts_bits, _) = buffer.data.iter().next_back()?;
                let stream_key = key.stream_key();
                let offset = self.inner.offsets.get(&stream_key).map_or(0.0, |o| *o.value());
                Some(f64::from_bits(*last_raw_ts_bits) + offset)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn get_interpolated_values_at_time(&self, keys: &[DataColumnId], time: f64) -> Vec<Option<f64>> {
        let mut interpolated_values = Vec::with_capacity(keys.len());

        for key in keys {
            let value = self.inner.buffers.get(key).and_then(|buffer_ref| {
                let offset = self.inner.offsets.get(&key.stream_key()).map_or(0.0, |off| *off.value());
                let target_time_bits = (time - offset).to_bits();

                let p1_opt = buffer_ref.data.range(..=target_time_bits).next_back();
                let p2_opt = buffer_ref.data.range(target_time_bits..).next();

                match (p1_opt, p2_opt) {
                    (Some((t1_bits, y1)), Some((t2_bits, y2))) => {
                        let p1 = Point::new(f64::from_bits(*t1_bits) + offset, *y1);
                        let p2 = Point::new(f64::from_bits(*t2_bits) + offset, *y2);
                        Some(lerp(&p1, &p2, time))
                    },
                    (Some((_, y1)), None) => Some(*y1),
                    (None, Some((_, y2))) => Some(*y2),
                    (None, None) => None,
                }
            });
            interpolated_values.push(value);
        }
        interpolated_values
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