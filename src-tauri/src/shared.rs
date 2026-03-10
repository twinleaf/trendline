//! trendline_lib/src/shared.rs
//! Front-end facing data shapes

use crate::util;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

use twinleaf::tio::proto::identifiers::ColumnKey as TwinleafColumnKey;
use twinleaf::tio::proto::meta::{
    ColumnMetadata as TwinleafColumnMetadata, DeviceMetadata as TwinleafDeviceMetadata,
    MetadataEpoch as TwinleafMetadataEpoch, MetadataFilter as TwinleafMetadataFilter,
    SegmentMetadata as TwinleafSegmentMetadata, StreamMetadata as TwinleafStreamMetadata,
};

use twinleaf::tio::proto::rpc::{
    RpcErrorCode as TwinleafRpcErrorCode, RpcErrorPayload as TwinleafRpcErrorPayload,
};

use twinleaf::tio::proxy::{PortError, RpcError as ProxyRpcError};

// FSM States -------------------------------------------------------------

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum PortState {
    Idle,
    Connecting,
    Discovery,
    Online,
    Reconnecting,
    Disconnected,
    Error(String),
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UiConfirmedSelection {
    pub port_url: String,
    pub children_routes: Vec<String>,
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UiSelectionState {
    pub active_port_url: Option<String>,
    pub confirmed: Option<UiConfirmedSelection>,
}

// Device -----------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq, Default)]
#[ts(export)]
pub struct UiDeviceMeta {
    pub serial_number: String,
    pub firmware_hash: String,
    pub n_streams: usize,
    pub session_id: u32,
    pub name: String,
}

impl From<TwinleafDeviceMetadata> for UiDeviceMeta {
    fn from(d: TwinleafDeviceMetadata) -> Self {
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
#[ts(export)]
pub struct UiStreamMeta {
    pub stream_id: u8,
    pub name: String,
    pub n_columns: usize,
    pub n_segments: usize,
    pub sample_size: usize,
    pub buf_samples: usize,
}

impl From<TwinleafStreamMetadata> for UiStreamMeta {
    fn from(s: TwinleafStreamMetadata) -> Self {
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
#[ts(export)]
pub struct UiColumnMeta {
    pub stream_id: u8,
    pub index: usize,
    pub data_type: String,
    pub name: String,
    pub units: String,
    pub description: String,
}

impl From<TwinleafColumnMetadata> for UiColumnMeta {
    fn from(c: TwinleafColumnMetadata) -> Self {
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
#[ts(export)]
pub struct UiSegmentMeta {
    pub stream_id: u8,
    pub segment_id: u8,
    pub flags: u8,
    pub time_ref_epoch: UiMetadataEpoch,
    pub time_ref_serial: String,
    pub time_ref_session_id: u32,
    pub start_time: u32,
    pub sampling_rate: u32,
    pub decimation: u32,
    pub filter_cutoff: f32,
    pub filter_type: UiMetadataFilter,
}

impl From<TwinleafSegmentMetadata> for UiSegmentMeta {
    fn from(s: TwinleafSegmentMetadata) -> Self {
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
#[ts(export)]
pub struct UiRpcMeta {
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
#[ts(export)]
pub enum UiRpcErrorCode {
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

impl From<TwinleafRpcErrorCode> for UiRpcErrorCode {
    fn from(lib_error: TwinleafRpcErrorCode) -> Self {
        match lib_error {
            TwinleafRpcErrorCode::NoError => UiRpcErrorCode::NoError,
            TwinleafRpcErrorCode::Undefined => UiRpcErrorCode::Undefined,
            TwinleafRpcErrorCode::NotFound => UiRpcErrorCode::NotFound,
            TwinleafRpcErrorCode::MalformedRequest => UiRpcErrorCode::MalformedRequest,
            TwinleafRpcErrorCode::WrongSizeArgs => UiRpcErrorCode::WrongSizeArgs,
            TwinleafRpcErrorCode::InvalidArgs => UiRpcErrorCode::InvalidArgs,
            TwinleafRpcErrorCode::ReadOnly => UiRpcErrorCode::ReadOnly,
            TwinleafRpcErrorCode::WriteOnly => UiRpcErrorCode::WriteOnly,
            TwinleafRpcErrorCode::Timeout => UiRpcErrorCode::Timeout,
            TwinleafRpcErrorCode::Busy => UiRpcErrorCode::Busy,
            TwinleafRpcErrorCode::WrongDeviceState => UiRpcErrorCode::WrongDeviceState,
            TwinleafRpcErrorCode::LoadFailed => UiRpcErrorCode::LoadFailed,
            TwinleafRpcErrorCode::LoadRpcFailed => UiRpcErrorCode::LoadRpcFailed,
            TwinleafRpcErrorCode::SaveFailed => UiRpcErrorCode::SaveFailed,
            TwinleafRpcErrorCode::SaveWriteFailed => UiRpcErrorCode::SaveWriteFailed,
            TwinleafRpcErrorCode::Internal => UiRpcErrorCode::Internal,
            TwinleafRpcErrorCode::OutOfMemory => UiRpcErrorCode::OutOfMemory,
            TwinleafRpcErrorCode::OutOfRange => UiRpcErrorCode::OutOfRange,
            TwinleafRpcErrorCode::Unknown(v) => UiRpcErrorCode::Unknown(v),
        }
    }
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UiRpcErrorPayload {
    pub id: u16,
    pub error: UiRpcErrorCode,
    pub extra: Vec<u8>,
}

impl From<TwinleafRpcErrorPayload> for UiRpcErrorPayload {
    fn from(s: TwinleafRpcErrorPayload) -> Self {
        Self {
            id: s.id,
            error: s.error.into(),
            extra: s.extra,
        }
    }
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type", content = "payload")]
pub enum UiRpcError {
    ExecError(UiRpcErrorPayload),
    SendFailed(String),
    RecvFailed(String),
    TypeError,
    AppLogic(String),
}
impl From<String> for UiRpcError {
    fn from(err: String) -> Self {
        UiRpcError::AppLogic(err)
    }
}

impl From<PortError> for UiRpcError {
    fn from(err: PortError) -> Self {
        UiRpcError::AppLogic(format!("Port Error: {:?}", err))
    }
}

impl From<ProxyRpcError> for UiRpcError {
    fn from(lib_err: ProxyRpcError) -> Self {
        match lib_err {
            ProxyRpcError::ExecError(payload) => UiRpcError::ExecError(payload.into()),

            ProxyRpcError::SendFailed(send_error) => {
                let msg = match send_error {
                    twinleaf::tio::proxy::SendError::WouldBlock(_) => {
                        "Send would block.".to_string()
                    }
                    twinleaf::tio::proxy::SendError::ProxyDisconnected(_) => {
                        "Proxy disconnected during send.".to_string()
                    }
                    twinleaf::tio::proxy::SendError::InvalidRoute(_) => {
                        "Invalid device route for send.".to_string()
                    }
                };
                UiRpcError::SendFailed(msg)
            }

            ProxyRpcError::RecvFailed(recv_error) => {
                let msg = match recv_error {
                    twinleaf::tio::proxy::RecvError::WouldBlock => {
                        "Receive would block.".to_string()
                    }
                    twinleaf::tio::proxy::RecvError::ProxyDisconnected => {
                        "Proxy disconnected during receive.".to_string()
                    }
                };
                UiRpcError::RecvFailed(msg)
            }
            ProxyRpcError::TypeError => UiRpcError::TypeError,
        }
    }
}

// Misc. Metadata -------------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum UiMetadataEpoch {
    Invalid,
    Zero,
    Systime,
    Unix,
    Unknown(u8),
}

impl From<TwinleafMetadataEpoch> for UiMetadataEpoch {
    fn from(e: TwinleafMetadataEpoch) -> Self {
        match e {
            TwinleafMetadataEpoch::Invalid => Self::Invalid,
            TwinleafMetadataEpoch::Zero => Self::Zero,
            TwinleafMetadataEpoch::Systime => Self::Systime,
            TwinleafMetadataEpoch::Unix => Self::Unix,
            TwinleafMetadataEpoch::Unknown(v) => Self::Unknown(v),
        }
    }
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum UiMetadataFilter {
    Unfiltered,
    FirstOrderCascade1,
    FirstOrderCascade2,
    Unknown(u8),
}

impl From<TwinleafMetadataFilter> for UiMetadataFilter {
    fn from(f: TwinleafMetadataFilter) -> Self {
        match f {
            TwinleafMetadataFilter::Unfiltered => Self::Unfiltered,
            TwinleafMetadataFilter::FirstOrderCascade1 => Self::FirstOrderCascade1,
            TwinleafMetadataFilter::FirstOrderCascade2 => Self::FirstOrderCascade2,
            TwinleafMetadataFilter::Unknown(v) => Self::Unknown(v),
        }
    }
}

// Frontend structs ----------------------------------------------------------
#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UiStream {
    pub meta: UiStreamMeta,
    pub segment: Option<UiSegmentMeta>,
    pub columns: Vec<UiColumnMeta>,
    pub effective_sampling_rate: f64,
}

#[derive(Serialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UiDevice {
    pub url: String,
    pub route: String,
    pub state: PortState,
    pub meta: UiDeviceMeta,
    pub streams: Vec<UiStream>,
    pub rpcs: Vec<UiRpcMeta>,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq, Default)]
#[ts(export)]
pub enum DecimationMethod {
    #[default]
    None,
    Fpcs,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq, Default)]
#[ts(export)]
pub enum DetrendMethod {
    #[default]
    None, // Only remove the mean (DC offset)
    Linear,    // Remove a linear trend (y = mt + c)
    Quadratic, // Remove a quadratic trend (y = at^2 + bt + c)
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum ViewConfig {
    Timeseries(TimeseriesConfig),
    Fft(FftConfig),
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq, Default)]
#[ts(export)]
pub enum PlotViewType {
    #[default]
    Timeseries,
    Fft,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct TimeseriesConfig {
    pub decimation_method: DecimationMethod,
    pub window_seconds: f64,
    pub resolution_multiplier: u32,
}
impl Default for TimeseriesConfig {
    fn default() -> Self {
        Self {
            decimation_method: DecimationMethod::Fpcs,
            window_seconds: 30.0,
            resolution_multiplier: 400,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct FftConfig {
    pub window_seconds: f64,
    pub detrend_method: DetrendMethod,
}
impl Default for FftConfig {
    fn default() -> Self {
        Self {
            window_seconds: 10.0,
            detrend_method: DetrendMethod::None,
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UiPlotState {
    pub plot_id: PlotId,
    pub data_keys: Vec<UiColumnKey>,
    pub max_sampling_rate: f64,
    pub active_view: PlotViewType,
    pub timeseries_config: TimeseriesConfig,
    pub fft_config: FftConfig,
}

impl UiPlotState {
    pub fn active_view_config(&self) -> ViewConfig {
        match self.active_view {
            PlotViewType::Timeseries => ViewConfig::Timeseries(self.timeseries_config.clone()),
            PlotViewType::Fft => ViewConfig::Fft(self.fft_config.clone()),
        }
    }
}

// Plot Structs -----------------------------------------------------------------

pub static VIRTUAL_ROUTE: std::sync::LazyLock<twinleaf::tio::proto::DeviceRoute> =
    std::sync::LazyLock::new(|| twinleaf::tio::proto::DeviceRoute::from_str("255").unwrap());

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UiColumnKey {
    #[serde(with = "util")]
    #[ts(type = "string")]
    pub route: twinleaf::tio::proto::DeviceRoute,
    pub stream_id: u8,
    pub column_id: usize,
}

impl UiColumnKey {
    pub fn new(route: twinleaf::tio::proto::DeviceRoute, stream_id: u8, column_id: usize) -> Self {
        Self {
            route,
            stream_id,
            column_id,
        }
    }

    pub fn device_key(&self) -> Self {
        Self {
            stream_id: 0,
            column_id: 0,
            ..self.clone()
        }
    }

    pub fn stream_key(&self) -> Self {
        Self {
            column_id: 0,
            ..self.clone()
        }
    }

    pub fn is_virtual(&self) -> bool {
        self.route == *VIRTUAL_ROUTE
    }
}

impl From<&UiColumnKey> for TwinleafColumnKey {
    fn from(value: &UiColumnKey) -> Self {
        Self::new(value.route.clone(), value.stream_id, value.column_id)
    }
}

impl From<UiColumnKey> for TwinleafColumnKey {
    fn from(value: UiColumnKey) -> Self {
        Self::from(&value)
    }
}

impl From<&TwinleafColumnKey> for UiColumnKey {
    fn from(value: &TwinleafColumnKey) -> Self {
        Self::new(value.route.clone(), value.stream_id, value.column_id)
    }
}

impl From<TwinleafColumnKey> for UiColumnKey {
    fn from(value: TwinleafColumnKey) -> Self {
        Self::from(&value)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, TS, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct PlotId(pub Uuid);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, TS, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct PipelineId(pub Uuid);

#[derive(Serialize, Deserialize, Clone, Debug, Default, TS, PartialEq)]
#[ts(export)]
pub struct PlotData {
    pub timestamps: Vec<f64>,
    pub series_data: Vec<Vec<Option<f64>>>,
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
    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
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

#[derive(Clone, Debug, Serialize, TS, Default, PartialEq)]
#[ts(export)]
pub struct StatisticSet {
    pub count: u64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub stdev: f64,
    pub rms: f64,
}

#[derive(Clone, Debug, Serialize, TS, Default, PartialEq)]
#[ts(export)]
pub struct HealthSet {
    pub gap_count: u64,
    pub nan_count: u64,
    pub gap_mean: f64,
    pub gap_min: f64,
    pub gap_max: f64,
}

#[derive(Clone, Debug, Serialize, TS, Default, PartialEq)]
#[ts(export)]
pub struct ColumnStatistics {
    pub latest_value: f64,
    pub persistent: StatisticSet,
    pub window: StatisticSet,
    pub persistent_health: HealthSet,
    pub window_health: HealthSet,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum BiquadKind {
    Lowpass,
    Highpass,
    Bandpass,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct BiquadParams {
    pub cutoff_hz: f64,
    pub q: f64, // for BPF: bandwidth via Q
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum MathChannelConfig {
    Arithmetic {
        a: UiColumnKey,
        op: MathOp,
        b: UiColumnKey,
    },
    Biquad {
        src: UiColumnKey,
        kind: BiquadKind,
        params: BiquadParams,
    },
}
