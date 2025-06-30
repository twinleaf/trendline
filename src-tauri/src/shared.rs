//! trendline_lib/src/shared.rs
//! Front-end facing data shapes

use serde::Serialize;
use ts_rs::TS;

// ─────────────────────────── 1. Port state ──────────────────────────────

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum PortState {
    Idle,
    Connecting,
    Discovery,
    Streaming,
    Reconnecting,
    Disconnected,
    Error(String),
}

// ───────────────────── 2.  Metadata mirror structs ──────────────────────
//

use twinleaf::tio::proto::meta::{
    ColumnMetadata as LibColumnMeta, DeviceMetadata as LibDeviceMeta,
    StreamMetadata as LibStreamMeta,
};

// Device -----------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct DeviceMeta {
    pub serial_number: String,
    pub firmware_hash: String,
    pub n_streams: usize,
    pub session_id: u32,
    pub name: String,
}

impl From<LibDeviceMeta> for DeviceMeta {
    fn from(d: LibDeviceMeta) -> Self {
        Self {
            serial_number: d.serial_number,
            firmware_hash: d.firmware_hash,
            n_streams: d.n_streams,
            session_id: d.session_id,
            name: d.name,
        }
    }
}

// Stream -----------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct StreamMeta {
    pub stream_id: u8,
    pub name: String,
    pub n_columns: usize,
    pub n_segments: usize,
    pub sample_size: usize,
    pub buf_samples: usize,
}

impl From<LibStreamMeta> for StreamMeta {
    fn from(s: LibStreamMeta) -> Self {
        Self {
            stream_id: s.stream_id,
            name: s.name,
            n_columns: s.n_columns,
            n_segments: s.n_segments,
            sample_size: s.sample_size,
            buf_samples: s.buf_samples,
        }
    }
}

// Column -----------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ColumnMeta {
    pub stream_id: u8,
    pub index: usize,
    pub data_type: String,
    pub name: String,
    pub units: String,
    pub description: String,
}

impl From<LibColumnMeta> for ColumnMeta {
    fn from(c: LibColumnMeta) -> Self {
        Self {
            stream_id: c.stream_id,
            index: c.index,
            data_type: format!("{:?}", c.data_type),
            name: c.name,
            units: c.units,
            description: c.description,
        }
    }
}

// ───────────────────── 3.  UI view-model structs ────────────────────────

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct UiStream {
    pub meta: StreamMeta,
    pub columns: Vec<ColumnMeta>,
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct UiDevice {
    pub url: String,
    pub route: String,
    pub state: PortState,
    pub meta: DeviceMeta,
    pub streams: Vec<UiStream>,
}

// ───────────────────────── 4.  PlotData slice ───────────────────────────

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct PlotData {
    pub timestamps: Vec<f64>,
    pub series_data: Vec<Vec<f64>>,
}

impl PlotData {
    pub fn empty() -> Self {
        Self {
            timestamps: vec![],
            series_data: vec![],
        }
    }
    pub fn with_series_capacity(n: usize) -> Self {
        Self {
            timestamps: vec![],
            series_data: vec![Vec::new(); n],
        }
    }
}
