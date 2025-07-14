//! trendline_lib/src/shared.rs
//! Front-end facing data shapes

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;
use num_enum::{FromPrimitive, IntoPrimitive};

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
    MetadataEpoch as LibMetadataEpoch, MetadataFilter as LibMetadataFilter,
};

use twinleaf::tio::proto::rpc::{
    RpcErrorCode as LibRpcErrorCode, RpcErrorPayload as LibRpcErrorPayload
};

use twinleaf::tio::proxy::{PortError, RpcError as LibProxyError};

// Device -----------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq, Default)]
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

// RPC -----------------------------------------------------------------
// TODO: ASK GB TO MOVE IT INTO TWINLEAF LIBRARY
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct RpcMeta {
    pub name: String,
    pub size: usize,
    pub permissions: String, // e.g., "RW-", "R-P"
    pub arg_type: String,    // e.g., "u32", "string<64>"
    pub readable: bool,
    pub writable: bool,
    pub persistent: bool,
    pub unknown: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub value: Option<Value>,
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[repr(u16)]
#[derive(FromPrimitive, IntoPrimitive)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum RpcErrorCode {
    NoError = 0,
    Undefined = 1,
    NotFound = 2,
    MalformedRequest = 3,
    WrongSizeArgs = 4,
    InvalidArgs = 5,
    ReadOnly = 6,
    WriteOnly = 7,
    Timeout = 8,
    Busy = 9,
    WrongDeviceState = 10,
    LoadFailed = 11,
    LoadRpcFailed = 12,
    SaveFailed = 13,
    SaveWriteFailed = 14,
    Internal = 15,
    OutOfMemory = 16,
    OutOfRange = 17,
    #[num_enum(catch_all)]
    Unknown(u16),
}

impl From<LibRpcErrorCode> for RpcErrorCode {
    fn from(lib_error: LibRpcErrorCode) -> Self {
        match lib_error {
            LibRpcErrorCode::NoError => RpcErrorCode::NoError,
            LibRpcErrorCode::Undefined => RpcErrorCode::Undefined,
            LibRpcErrorCode::NotFound => RpcErrorCode::NotFound,
            LibRpcErrorCode::MalformedRequest => RpcErrorCode::MalformedRequest,
            LibRpcErrorCode::WrongSizeArgs => RpcErrorCode::WrongSizeArgs,
            LibRpcErrorCode::InvalidArgs => RpcErrorCode::InvalidArgs,
            LibRpcErrorCode::ReadOnly => RpcErrorCode::ReadOnly,
            LibRpcErrorCode::WriteOnly => RpcErrorCode::WriteOnly,
            LibRpcErrorCode::Timeout => RpcErrorCode::Timeout,
            LibRpcErrorCode::Busy => RpcErrorCode::Busy,
            LibRpcErrorCode::WrongDeviceState => RpcErrorCode::WrongDeviceState,
            LibRpcErrorCode::LoadFailed => RpcErrorCode::LoadFailed,
            LibRpcErrorCode::LoadRpcFailed => RpcErrorCode::LoadRpcFailed,
            LibRpcErrorCode::SaveFailed => RpcErrorCode::SaveFailed,
            LibRpcErrorCode::SaveWriteFailed => RpcErrorCode::SaveWriteFailed,
            LibRpcErrorCode::Internal => RpcErrorCode::Internal,
            LibRpcErrorCode::OutOfMemory => RpcErrorCode::OutOfMemory,
            LibRpcErrorCode::OutOfRange => RpcErrorCode::OutOfRange,
            LibRpcErrorCode::Unknown(v) => RpcErrorCode::Unknown(v),
        }
    }
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct RpcErrorPayload {
    pub id: u16,
    pub error: RpcErrorCode,
    pub extra: Vec<u8>,
}

impl From<LibRpcErrorPayload> for RpcErrorPayload {
    fn from(s: LibRpcErrorPayload ) -> Self {
        Self {
            id: s.id,
            error: s.error.into(),
            extra: s.extra,
        }
    }
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export, export_to = "../../src/lib/bindings/")]
#[serde(tag = "type", content = "payload")]
pub enum RpcError {
    ExecError(RpcErrorPayload), 
    SendFailed(String),
    RecvFailed(String),
    TypeError,
    AppLogic(String),
}
impl From<String> for RpcError {
    fn from(err: String) -> Self {
        RpcError::AppLogic(err)
    }
}

impl From<PortError> for RpcError {
    fn from(err: PortError) -> Self {
        RpcError::AppLogic(format!("Port Error: {:?}", err))
    }
}

impl From<LibProxyError> for RpcError {
    fn from(lib_err: LibProxyError) -> Self {
        match lib_err {
            LibProxyError::ExecError(payload) => RpcError::ExecError(payload.into()),
            
            LibProxyError::SendFailed(send_error) => {
                let msg = match send_error {
                    twinleaf::tio::proxy::SendError::WouldBlock(_) => "Send would block.".to_string(),
                    twinleaf::tio::proxy::SendError::ProxyDisconnected(_) => "Proxy disconnected during send.".to_string(),
                    twinleaf::tio::proxy::SendError::InvalidRoute(_) => "Invalid device route for send.".to_string(),
                };
                RpcError::SendFailed(msg)
            },
            
            LibProxyError::RecvFailed(recv_error) => {
                let msg = match recv_error {
                    twinleaf::tio::proxy::RecvError::WouldBlock => "Receive would block.".to_string(),
                    twinleaf::tio::proxy::RecvError::ProxyDisconnected => "Proxy disconnected during receive.".to_string(),
                };
                RpcError::RecvFailed(msg)
            },
            LibProxyError::TypeError => RpcError::TypeError,
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
    pub rpcs: Vec<RpcMeta>,
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
