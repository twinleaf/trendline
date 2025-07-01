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
    StreamMetadata as LibStreamMeta, SegmentMetadata as LibSegmentMeta,
    MetadataEpoch as LibMetadataEpoch, MetadataFilter as LibMetadataFilter
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

// Segment ----------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct SegmentMeta {
    pub stream_id: u8,
    pub segment_id: u8,
    pub flags: u8,
    pub time_ref_epoch: MetadataEpoch,
    pub time_ref_serial: String,
    pub time_ref_session_id: u32,
    pub start_time: u32,
    pub sampling_rate: u32,
    pub decimation: u32,
    pub filter_cutoff: f32,
    pub filter_type: MetadataFilter,
}

impl From<LibSegmentMeta> for SegmentMeta {
    fn from(s: LibSegmentMeta) -> Self {
        Self {
            stream_id: s.stream_id,
            segment_id: s.segment_id,
            flags: s.flags,
            time_ref_epoch: s.time_ref_epoch.into(),
            time_ref_serial: s.time_ref_serial,
            time_ref_session_id: s.time_ref_session_id,
            start_time: s.start_time,
            sampling_rate: s.sampling_rate,
            decimation: s.decimation,
            filter_cutoff: s.filter_cutoff,
            filter_type: s.filter_type.into(),
        }
    }
}

// --- Enums ---
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum MetadataEpoch {
    Invalid, Zero, Systime, Unix, Unknown(u8),
}

impl From<LibMetadataEpoch> for MetadataEpoch {
    fn from(e: LibMetadataEpoch) -> Self {
        match e {
            LibMetadataEpoch::Invalid => Self::Invalid,
            LibMetadataEpoch::Zero => Self::Zero,
            LibMetadataEpoch::Systime => Self::Systime,
            LibMetadataEpoch::Unix => Self::Unix,
            LibMetadataEpoch::Unknown(v) => Self::Unknown(v),
        }
    }
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum MetadataFilter {
    Unfiltered, FirstOrderCascade1, FirstOrderCascade2, Unknown(u8),
}

impl From<LibMetadataFilter> for MetadataFilter {
    fn from(f: LibMetadataFilter) -> Self {
        match f {
            LibMetadataFilter::Unfiltered => Self::Unfiltered,
            LibMetadataFilter::FirstOrderCascade1 => Self::FirstOrderCascade1,
            LibMetadataFilter::FirstOrderCascade2 => Self::FirstOrderCascade2,
            LibMetadataFilter::Unknown(v) => Self::Unknown(v),
        }
    }
}



// ───────────────────── 3.  UI view-model structs ────────────────────────

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct UiStream {
    pub meta: StreamMeta,
    pub segment: Option<SegmentMeta>,
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
