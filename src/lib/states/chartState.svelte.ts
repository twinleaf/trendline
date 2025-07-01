import { deviceState } from '$lib/states/deviceState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { UiDevice } from '$lib/bindings/UiDevice';


export type ChartLayout = 'carousel' | 'vertical' | 'horizontal';

export type StreamLayout = 'grouped' | 'vertical' | 'horizontal';

export interface PlotSeries {
	dataKey: DataColumnId; 
	uPlotSeries: uPlot.Series;
}

export interface PlotConfig {
	title: string;
	series: PlotSeries[];

	uPlotOptions: uPlot.Options;
}


export interface DevicePlots {
	device: UiDevice;
	plots: PlotConfig[];
}


class ChartState {
	// --- User-configurable settings ---
	chartLayout = $state<ChartLayout>('vertical');
	streamLayout = $state<StreamLayout>('grouped');

	// This is the primary output of our state. The UI will just `{#each}` over this.
	renderPlan = $derived(this.#calculateRenderPlan());


	#calculateRenderPlan(): DevicePlots[] {
		const selectedDevices = deviceState.selectedDevices;
		if (selectedDevices.length === 0) {
			return [];
		}

		const plan: DevicePlots[] = [];

		for (const device of selectedDevices) {
			const devicePlots: DevicePlots = {
				device: device,
				plots: this.#generatePlotsForDevice(device)
			};
			plan.push(devicePlots);
		}
        
        plan.sort((a, b) => {
            return a.device.route.localeCompare(b.device.route, undefined, { numeric: true });
        });

		return plan;
	}

	#generatePlotsForDevice(device: UiDevice): PlotConfig[] {
		switch (this.streamLayout) {
			case 'grouped':
				return this.#generateGroupedPlots(device);
			case 'vertical':
				// TODO: Implement vertical stream layout
				return [];
			case 'horizontal':
				// TODO: Implement horizontal stream layout
				return [];
			default:
				return [];
		}
	}

	#generateGroupedPlots(device: UiDevice): PlotConfig[] {
		const plotsByUnit = new Map<string, PlotSeries[]>();

		for (const stream of device.streams) {
			for (const col of stream.columns) {
				const unit = col.units || 'unknown';
				
				const plotSeries: PlotSeries = {

					dataKey: {
						port_url: device.url,
						device_route: device.route,
						stream_id: stream.meta.stream_id,
						column_index: col.index,
					},
					uPlotSeries: {
						label: `${stream.meta.name} - ${col.name}`, 
						stroke: this.#getColorForSeries(),
                        pxAlign: 0,
					}
				};

				const seriesForUnit = plotsByUnit.get(unit) ?? [];
				seriesForUnit.push(plotSeries);
				plotsByUnit.set(unit, seriesForUnit);
			}
		}

		const finalPlots: PlotConfig[] = [];
		for (const [unit, series] of plotsByUnit.entries()) {
			const plotConfig: PlotConfig = {
				title: `Measurements in ${unit}`,
				series: series,
				uPlotOptions: this.#createUplotOptions(series, unit)
			};
			finalPlots.push(plotConfig);
		}

		return finalPlots;
	}
    
    #createUplotOptions(series: PlotSeries[], primaryUnit: string): uPlot.Options {

        const uplotSeriesConfig: uPlot.Series[] = [
            {},
            ...series.map(s => s.uPlotSeries)
        ];

        return {
            width: 800, // This will be overridden by the component's width
            height: 400, // This will be overridden by the component's height
            series: uplotSeriesConfig,
            pxAlign: 0,
            axes: [
                {}, // Default X-axis
                {   // Default Y-axis
                    scale: primaryUnit,
                    label: primaryUnit,
                    values: (u, vals) => vals.map(v => v.toFixed(2)),
                }
            ],
            scales: {
                [primaryUnit]: {
                    auto: true, // Automatically scale the Y-axis
                }
            },
            cursor: {
                drag: {
                    x: true,
                    y: true,
                }
            }
        };
    }

	#colorIdx = 0;
	#colors = ['#FF0000', '#00FF00', '#0000FF', '#FFFF00', '#00FFFF', '#FF00FF'];
	#getColorForSeries(): string {
		const color = this.#colors[this.#colorIdx % this.#colors.length];
		this.#colorIdx++;
		return color;
	}
}

export const chartState = new ChartState();