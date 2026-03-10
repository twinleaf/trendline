use super::{Pipeline, PipelineCommand};
use crate::shared::{PipelineId, PlotData};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use welch_sde::{Build, SpectralDensity};

pub struct FftPipeline {
    id: PipelineId,
    output: Arc<Mutex<PlotData>>,
}

impl FftPipeline {
    pub fn new() -> Self {
        Self {
            id: PipelineId(Uuid::new_v4()),
            output: Arc::new(Mutex::new(PlotData::empty())),
        }
    }
}

impl Pipeline for FftPipeline {
    fn id(&self) -> PipelineId {
        self.id
    }

    fn get_output(&self) -> PlotData {
        self.output.lock().unwrap().clone()
    }

    fn process_derived_batch(&mut self, batch: (PlotData, f64)) {
        let (plot_data, sample_rate) = batch;

        let raw_series = if let Some(data) = plot_data.series_data.get(0) {
            data
        } else {
            return;
        };

        // Filter out None values (gaps) — FFT needs a continuous block
        let y_values: Vec<f64> = raw_series.iter().filter_map(|&v| v).collect();

        if y_values.len() < 16 || sample_rate <= 0.0 {
            *self.output.lock().unwrap() = PlotData::empty();
            return;
        }

        let welch: SpectralDensity<f64> = SpectralDensity::builder(&y_values, sample_rate).build();
        let psd = welch.periodogram();

        let asd: Vec<Option<f64>> = psd.to_vec().iter().map(|&p| Some(p.sqrt())).collect();
        let frequencies = psd.frequency().to_vec();

        let result = PlotData {
            timestamps: frequencies,
            series_data: vec![asd],
        };

        *self.output.lock().unwrap() = result;
    }

    fn process_command(&mut self, cmd: PipelineCommand) {
        if let PipelineCommand::ResetSelf = cmd {
            *self.output.lock().unwrap() = PlotData::empty();
        }
    }
}
