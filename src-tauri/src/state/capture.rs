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
                default_cap: 1_200_000, // e.g., 1000 samples/sec * 60 sec * 20 min
            }),
        }
    }

    pub fn get_all_data_for_keys(&self, keys: &[DataColumnId]) -> PlotData {
        if keys.is_empty() {
            return PlotData::empty();
        }

        // Create a PlotData object with the correct number of series.
        let mut plot_data = PlotData::with_series_capacity(keys.len());

        // We'll build a unified, sorted list of all timestamps from all series.
        let mut all_timestamps: BTreeMap<u64, ()> = BTreeMap::new();
        
        // Collect references to all the relevant buffers first to minimize lock time.
        let buffers: Vec<_> = keys
            .iter()
            .filter_map(|key| self.inner.buffers.get(key))
            .collect();
        
        for buffer_ref in &buffers {
            for ts_bits in buffer_ref.data.keys() {
                all_timestamps.insert(*ts_bits, ());
            }
        }

        // Populate the timestamps array in the final PlotData object.
        plot_data.timestamps = all_timestamps.keys().map(|&bits| f64::from_bits(bits)).collect();


        for (i, buffer_ref) in buffers.iter().enumerate() {
            let series_y_values = &mut plot_data.series_data[i];
            series_y_values.reserve(all_timestamps.len());

            for ts_bits in all_timestamps.keys() {
                if let Some(y_value) = buffer_ref.data.get(ts_bits) {
                    series_y_values.push(*y_value);
                } else {
                    series_y_values.push(f64::NAN);
                }
            }
        }
        
        plot_data
    }

    /// Inserts a data point for a given column, but only if that column is active.
    pub fn insert(&self, key: &DataColumnId, p: Point) {
        if !self.inner.active.contains_key(key) {
            return;
        }
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
    }
}
