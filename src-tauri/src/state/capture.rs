use dashmap::DashMap;
use std::collections::BTreeMap;
use std::sync::Arc;
use twinleaf::tio::proto::DeviceRoute; // Assuming you have this use statement

// The unique identifier for a single column of data from a specific device.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DataColumnId {
    pub port_url: String,
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

    /// Inserts a data point for a given column, but only if that column is active.
    /// This is the primary function called from the data-receiving loop in `PortManager`.
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