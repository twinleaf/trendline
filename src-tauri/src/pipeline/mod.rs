use crate::shared::{PipelineId, PlotData, StreamStatistics};
use crate::state::capture::CaptureState;
pub trait Pipeline: Send + Sync {
    fn id(&self) -> PipelineId;
    fn get_output(&self) -> PlotData;
    fn update(&mut self, capture_state: &CaptureState);
    fn get_source_sampling_rate(&self, capture_state: &CaptureState) -> Option<f64>;
}

pub trait StatisticsProvider: Send + Sync {
    fn id(&self) -> PipelineId;
    fn get_output(&self) -> StreamStatistics;
    fn update(&mut self, capture_state: &CaptureState);
    fn reset(&mut self, capture_state: &CaptureState);
}

pub mod buffer;
pub mod decimation;
pub mod detrend;
pub mod fft;
pub mod manager;
pub mod passthrough;
pub mod statistics;
