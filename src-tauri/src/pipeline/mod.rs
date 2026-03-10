use crate::shared::{ColumnStatistics, PipelineId, PlotData, Point};
use crate::state::data_manager::BatchedData;
use crossbeam::channel::Sender;
use std::sync::Arc;

#[derive(Debug)]
pub enum PipelineCommand {
    Hydrate {
        sampling_rate: Option<f64>,
        backfill: Vec<Point>,
    },
    AddSubscriber(Sender<(PlotData, f64)>),
    Shutdown,
    ResetSelf,
}

/// The core trait for a processing stage.
pub trait Pipeline: Send + Sync {
    fn id(&self) -> PipelineId;
    fn get_output(&self) -> PlotData;
    fn process_batch(&mut self, _batch: Arc<BatchedData>) {}
    fn process_derived_batch(&mut self, _batch: (PlotData, f64)) {}
    fn process_command(&mut self, cmd: PipelineCommand);
}

pub trait StatisticsProvider: Send + Sync {
    fn id(&self) -> PipelineId;
    fn get_output(&mut self) -> ColumnStatistics;
    fn process_batch(&mut self, batch: Arc<BatchedData>);
    fn reset(&mut self);
}
pub mod arithmetic;
pub mod decimation;
pub mod detrend;
pub mod fft;
pub mod filtering;
pub mod manager;
pub mod passthrough;
pub mod statistics;
