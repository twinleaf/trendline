import { deviceState } from '$lib/states/deviceState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { UiDevice } from '$lib/bindings/UiDevice';
import { SvelteMap } from 'svelte/reactivity';


export type ChartLayout = 'carousel' | 'vertical' | 'horizontal';
export type StreamLayout = 'grouped' | 'vertical' | 'horizontal';

export interface PlotSeries {
	dataKey: DataColumnId; 
	uPlotSeries: uPlot.Series;
}

export class PlotConfig {
    title = $state('New Plot');
    series = $state<PlotSeries[]>([]);
    uPlotOptions: uPlot.Options;
    
    hasData = $state(false);

    constructor(title: string, series: PlotSeries[]) {
        this.title = title;
        this.series = series;
    }
}


export interface DevicePlots {
	device: UiDevice;
	plots: PlotConfig[];
}


class ChartState {
	// --- User-configurable settings ---
	chartLayout = $state<ChartLayout>('vertical');
	streamLayout = $state<StreamLayout>('grouped');
    #plotConfigs = new SvelteMap<string, PlotConfig>();

	renderPlan = $derived.by(() => this.#assembleRenderPlan());

    constructor() {
        $effect(() => {
            this.#updatePlotConfigs();
        });
    }

     #updatePlotConfigs(): void {
        const selectedDevices = deviceState.selectedDevices;
        if (selectedDevices.length === 0) {
            this.#plotConfigs.clear();
            return;
        }

        const requiredKeys = new Set<string>();

        for (const device of selectedDevices) {
            const plots = this.#generatePlotsForDevice(device, (plotConfig, unit) => {
                const key = `${device.route}:${unit}`;
                requiredKeys.add(key);

                if (!this.#plotConfigs.has(key)) {
                    this.#plotConfigs.set(key, plotConfig);
                }
            });
        }
        
        for (const key of this.#plotConfigs.keys()) {
            if (!requiredKeys.has(key)) {
                this.#plotConfigs.delete(key);
            }
        }
    }


	#assembleRenderPlan(): DevicePlots[] {
        const selectedDevices = deviceState.selectedDevices;
        if (selectedDevices.length === 0) {
            return [];
        }
        const planByDevice = new Map<UiDevice, PlotConfig[]>();

        for (const [key, plotConfig] of this.#plotConfigs.entries()) {
            const deviceRoute = key.split(':')[0];
            const device = deviceState.selectedDevices.find(d => d.route === deviceRoute);
            
            if (device) {
                const plots = planByDevice.get(device) ?? [];
                plots.push(plotConfig);
                planByDevice.set(device, plots);
            }
        }
        
        const finalPlan: DevicePlots[] = Array.from(planByDevice.entries()).map(([device, plots]) => ({
            device,
            plots
        }));

        finalPlan.sort((a, b) => a.device.route.localeCompare(b.device.route, undefined, { numeric: true }));

        return finalPlan;
    }

    #generatePlotsForDevice(device: UiDevice, onPlotCreated: (plot: PlotConfig, unit: string) => void): void {
        switch (this.streamLayout) {
            case 'grouped':
                this.#generateGroupedPlots(device, onPlotCreated);
                return; 
            case 'vertical':
                // TODO: Implement vertical stream layout
                return; 
            case 'horizontal':
                // TODO: Implement horizontal stream layout
                return; 
            default:
                return;
        }
    }

    #generateGroupedPlots(device: UiDevice, onPlotCreated: (plot: PlotConfig, unit: string) => void): void {
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
						label: `${col.name}`, 
						stroke: this.#getColorForSeries(),
                        scale: `${col.units}`,
                        pxAlign: 0,
					}
				};

				const seriesForUnit = plotsByUnit.get(unit) ?? [];
				seriesForUnit.push(plotSeries);
				plotsByUnit.set(unit, seriesForUnit);
			}
		}

		for (const [unit, series] of plotsByUnit.entries()) {
            const plotTitle = `${unit}`;

			const plotConfig = new PlotConfig(
                plotTitle,
                series,
                this.#createUplotOptions(plotTitle, series, unit)
            );
			onPlotCreated(plotConfig, unit);
		}

	}
    
    #createUplotOptions(title: string, series: PlotSeries[], primaryUnit: string): uPlot.Options {

        const uplotSeriesConfig: uPlot.Series[] = [
            {},
            ...series.map(s => s.uPlotSeries)
        ];

        return {
            width: 800, // This will be overridden by the component's width
            height: 400, // This will be overridden by the component's height
            series: uplotSeriesConfig,
            pxAlign: 0,
            legend: {
                show: true,
            },
            axes: [
                {}, // Default X-axis
                {   // Default Y-axis
                    scale: primaryUnit,
                    label: primaryUnit,
                    labelGap: 20,
                    labelSize: 40,
                    values: (u, vals) => {
                        const scale = u.scales[primaryUnit];

                        if (!scale || scale.min == null || scale.max == null) {
                            return vals.map(v => v.toFixed(2) + " ");
                        }

                        const range = scale.max - scale.min;

                        let decimals;
                        if (range <= 0) {
                            decimals = 2; // Default for flat data
                        } else if (range < 1) {
                            decimals = 3; // High precision for small ranges (e.g., 1.503)
                        } else if (range < 100) {
                            decimals = 2; // Standard precision
                        } else {
                            decimals = 0; // No decimals for large ranges (e.g., 26800)
                        }

                        return vals.map(v => v.toFixed(decimals) + " ");
                    },
                }
            ],
            cursor: {
            drag: {
                setScale: false,
                x: false,
                y: false,
                }
            },
            select: {
                show: false
                ,
                left: 0,
                top: 0,
                width: 0,
                height: 0
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