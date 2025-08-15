use crate::shared::{PipelineId, PlotData, StreamStatistics};
use crate::state::capture::{BatchedData, CaptureState};
use crossbeam::channel::Sender;
use std::sync::Arc;

#[derive(Debug)]
pub enum PipelineCommand {
    Hydrate,
    AddSubscriber(Sender<(PlotData, f64)>),
    Shutdown,
}

/// The core trait for a processing stage.
pub trait Pipeline: Send + Sync {
    fn id(&self) -> PipelineId;
    fn get_output(&self) -> PlotData;
    fn process_batch(&mut self, _batch: Arc<BatchedData>) {}
    fn process_derived_batch(&mut self, _batch: (PlotData, f64)) {}
    fn process_command(&mut self, cmd: PipelineCommand, capture: &CaptureState);
}

pub trait StatisticsProvider: Send + Sync {
    fn id(&self) -> PipelineId;
    fn get_output(&mut self, capture_state: &CaptureState) -> StreamStatistics;
    fn process_batch(&mut self, batch: Arc<BatchedData>); 
    fn reset(&mut self, capture_state: &CaptureState);
}
pub mod decimation;
pub mod detrend;
pub mod fft;
pub mod manager;
pub mod passthrough;
pub mod statistics;