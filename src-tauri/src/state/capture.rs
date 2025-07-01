use dashmap::DashMap;
use serde::Deserialize;
use ts_rs::TS;
use std::collections::BTreeMap;
use std::sync::Arc;
use twinleaf::tio::proto::DeviceRoute;
use crate::util;

use crate::shared::PlotData; // Assuming you have this use statement

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

#[derive(Clone, Debug)]
pub struct Point {
    pub t: f64,
    pub y: f64,
}

// A circular buffer for a single time series.
struct Buffer {
    data: BTreeMap<u64, f64>,
    cap: usize,
}

impl Buffer {
    fn new(cap: usize) -> Self {
        Self {
            data: BTreeMap::new(),
            cap,
        }
    }

    fn push(&mut self, p: Point) {
        self.data.insert(p.t.to_bits(), p.y);

        if self.data.len() > self.cap {
            if let Some(oldest_key) = self.data.keys().next().copied() {
                self.data.remove(&oldest_key);
            }
        }
    }
}

// The shared data that needs to be accessed by multiple threads.
struct Inner {
    buffers: DashMap<DataColumnId, Buffer>,
    active: DashMap<DataColumnId, ()>,
    offsets: DashMap<DataColumnId, f64>,
    default_cap: usize,
}

#[derive(Clone)]
pub struct CaptureState {
    inner: Arc<Inner>,
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

    pub fn get_data_in_range(&self, keys: &[DataColumnId], min_time: f64, max_time: f64) -> PlotData {
        if keys.is_empty() || min_time >= max_time {
            return PlotData::empty();
        }

        let stream_key = DataColumnId { column_index: 0, ..keys[0].clone() };
        let offset = match self.inner.offsets.get(&stream_key) {
            Some(off) => *off.value(),
            None => return PlotData::empty(),
        };
        let min_key = (min_time - offset).to_bits();
        let max_key = (max_time - offset).to_bits();

        let buffers: Vec<_> = keys
            .iter()
            .filter_map(|key| self.inner.buffers.get(key).map(|guard| guard.data.clone()))
            .collect();

        if buffers.is_empty() {
            return PlotData::empty();
        }

        let mut series_iters: Vec<_> = buffers
            .iter()
            .map(|buffer| buffer.range(min_key..=max_key).peekable())
            .collect();

        let num_series = series_iters.len();
        let mut plot_data = PlotData::with_series_capacity(num_series);

        loop {
            // 2. Find the smallest timestamp among the current heads of all iterators.
            let next_ts_bits = series_iters
                .iter_mut()
                .filter_map(|it| it.peek().map(|(&ts, _y)| ts))
                .min();

            if let Some(ts_bits) = next_ts_bits {
                let relative_timestamp = f64::from_bits(ts_bits) + offset;
                plot_data.timestamps.push(relative_timestamp);

                // 3. For each series, if its head matches the smallest timestamp,
                //    consume the point and push the value. Otherwise, push NaN.
                for (i, iter) in series_iters.iter_mut().enumerate() {
                    let y_val = if let Some((&ts, &y)) = iter.peek() {
                        if ts == ts_bits {
                            iter.next(); // Consume the point
                            y
                        } else {
                            f64::NAN
                        }
                    } else {
                        f64::NAN
                    };
                    // This assumes your PlotData is pre-initialized with empty Vecs
                    plot_data.series_data[i].push(y_val);
                }
            } else {
                // All iterators are exhausted.
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
                key.port_url, key.device_route, key.stream_id, p.t
            );
            -p.t // The offset is the negative of the first point's timestamp.
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
