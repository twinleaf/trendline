// src-tauri/src/app_state.rs
use crate::discover::FeDeviceMeta;
use twinleaf::tio::proxy::Port;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use ts_rs::TS;

/// The main storage for all time-series data.
pub type TimeSeriesData = HashMap<String, BTreeMap<u64, f64>>;

#[derive(serde::Serialize, Clone, Debug, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct SinglePlotPoint {
    pub x: f64,
    pub y: f64,
    pub series_key: String,
}
pub struct BackendState {
    pub proxies: HashMap<String, Port>,
    pub device_tree: HashMap<String, FeDeviceMeta>,
}

impl Default for BackendState {
    fn default() -> Self {
        Self {
            proxies: HashMap::new(),
            device_tree: HashMap::new(),
        }
    }
}

/// AppState holds the shared state for the application.
#[derive(Clone)]
pub struct AppState {
    pub buffer: Arc<Mutex<TimeSeriesData>>,
    pub backend_state: Arc<Mutex<BackendState>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(TimeSeriesData::default())),
            backend_state: Arc::new(Mutex::new(BackendState::default())),
        }
    }
}