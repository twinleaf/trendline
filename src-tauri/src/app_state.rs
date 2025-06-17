// src-tauri/src/app_state.rs

use ringbuf::HeapRb;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(serde::Serialize, Clone, Default, Debug)]
pub struct PlotDataPoint {
    pub timestamp: f64,
    pub values: HashMap<String, f64>,
}


const RING_BUFFER_CAPACITY: usize = 5000;

pub struct AppState {
    pub buffer: Mutex<HeapRb<PlotDataPoint>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            buffer: Mutex::new(HeapRb::new(RING_BUFFER_CAPACITY)),
        }
    }
}