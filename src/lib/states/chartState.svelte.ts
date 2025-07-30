import { deviceState } from '$lib/states/deviceState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { DecimationMethod } from '$lib/bindings/DecimationMethod';
import type { DetrendMethod } from '$lib/bindings/DetrendMethod';
import type { RowSelectionState } from '@tanstack/table-core';
import type { ExpandedState } from '@tanstack/table-core';
import { untrack } from 'svelte';

export type ChartLayout = 'carousel' | 'vertical' | 'horizontal';
export type StreamLayout = 'grouped' | 'vertical' | 'horizontal';

export interface PlotSeries {
	dataKey: DataColumnId;
	uPlotSeries: uPlot.Series;
}

class DataColumnStyler {
	#styles = new Map<string, { color: string }>();
	#colors = [
		'#3498db',
		'#e74c3c',
		'#2ecc71',
		'#f1c40f',
		'#9b59b6',
		'#1abc9c',
		'#d35400',
		'#34495e',
		'#e67e22',
		'#16a085',
		'#c0392b',
		'#8e44ad'
	];
	#colorIndex = 0;

	#getNextColor(): string {
		const color = this.#colors[this.#colorIndex % this.#colors.length];
		this.#colorIndex++;
		return color;
	}
	getStyle(dataKey: DataColumnId): { color: string } {
		const key = JSON.stringify(dataKey);
		if (!this.#styles.has(key)) {
			this.#styles.set(key, { color: this.#getNextColor() });
		}
		return this.#styles.get(key)!;
	}
}

export const dataColumnStyler = new DataColumnStyler();

export class PlotConfig {
	id = crypto.randomUUID();
	title = $state('New Plot');
	rowSelection = $state<RowSelectionState>({});
	expansion = $state<ExpandedState>({});
	activeTab = $state<'selection' | 'settings'>('selection');

	#manualDecimationMethod = $state<DecimationMethod>('Fpcs');
	#isDecimationManual = $state(false);
	windowSeconds = $state<number>(30.0);
	resolutionMultiplier = $state<number>(100);

	fftSeconds = $state<number>(10.0);
	fftYAxisDecades = $state(4);
	fftDetrendMethod = $state<DetrendMethod>('None');

	hasData = $state(false);
	latestTimestamp = $state(0);
	isPaused = $state(false);

	get decimationMethod(): DecimationMethod {
		if (this.#isDecimationManual) {
			return this.#manualDecimationMethod;
		}
		return this.series.length > 1 ? 'MinMax' : 'Fpcs';
	}

	set decimationMethod(value: DecimationMethod) {
		this.#isDecimationManual = true;
		this.#manualDecimationMethod = value;
	}

	series = $derived.by((): PlotSeries[] => {
		const newSeries: PlotSeries[] = [];
		const selectedKeys = Object.keys(this.rowSelection);
		for (const keyString of selectedKeys) {
			if (keyString.startsWith('{') && keyString.endsWith('}')) {
				try {
					const dataKey: DataColumnId = JSON.parse(keyString);

					const device = deviceState.getDevice(dataKey.port_url, dataKey.device_route);
					const stream = device?.streams.find((s) => s.meta.stream_id === dataKey.stream_id);
					const column = stream?.columns.find((c) => c.index === dataKey.column_index);

					if (device && stream && column) {
						const style = dataColumnStyler.getStyle(dataKey);
						newSeries.push({
							dataKey: dataKey,
							uPlotSeries: {
								label: column.name,
								stroke: style.color,
								scale: column.units
							}
						});
					}
				} catch (e) {
					console.error(
						'This should not happen. Failed to parse a key that looked like JSON:',
						keyString,
						e
					);
				}
			}
		}
		return newSeries;
	});

	viewType = $state<'timeseries' | 'fft'>('timeseries');

	maxSamplingRate = $derived.by((): number => {
		if (this.series.length === 0) {
			return 0;
		}
		const rates = this.series.map((s) => {
			const device = deviceState.getDevice(s.dataKey.port_url, s.dataKey.device_route);
			const stream = device?.streams.find((st) => st.meta.stream_id === s.dataKey.stream_id);
			return stream?.effective_sampling_rate ?? 0;
		});
		return Math.max(...rates);
	});

	uPlotOptions = $derived.by((): uPlot.Options => {
		if (this.series.length === 0) {
			return {
				width: 800,
				height: 400,
				legend: { show: false },
				series: [{}],
				axes: [{}, { show: false }]
			};
		}

		const uniqueUnits = new Set(this.series.map((s) => s.uPlotSeries.scale));


		const scalesConfig: Record<string, uPlot.Scale> = {};
		for (const unit of uniqueUnits) {
			if (unit) {
				if (this.viewType === 'fft') {
					scalesConfig[unit] = {
						auto: true,
						range: (u, dataMin, dataMax): [number | null, number | null] => {
							if (dataMin <= 0 || !isFinite(dataMin)) {
								return [1e-4, 1];
							}
							
							// Part 1: Make the range "sticky" by reading the current state
							const currentViewMin = u.scales[unit]?.min ?? null;
							const currentViewMax = u.scales[unit]?.max ?? null;

							let shouldResize = false;

							if (currentViewMin === null || currentViewMax === null) {
								shouldResize = true;
							} else {
								// Must resize if data goes out of the current view
								if (dataMin < currentViewMin || dataMax > currentViewMax) {
									shouldResize = true;
								} else {
									// Hysteresis: Only shrink the view if it's significantly larger than the data
									const viewDecades = Math.log10(currentViewMax) - Math.log10(currentViewMin);
									const dataDecades = Math.log10(dataMax) - Math.log10(dataMin);
									
									// Shrink only if view is >2 decades larger than what's needed
									const SHRINK_HYSTERESIS_DECADES = 2.0; 
									if (viewDecades > dataDecades + SHRINK_HYSTERESIS_DECADES) {
										shouldResize = true;
									}
								}
							}

							if (!shouldResize) {
								// If no resize is triggered, persist the current range to prevent jumping.
								return [currentViewMin, currentViewMax];
							}

							// Part 2: If a resize is needed, use the clean, symmetric calculation
							if (dataMin === dataMax) {
								const logVal = Math.log10(dataMin);
								return [10 ** (logVal - 1), 10 ** (logVal + 1)];
							}

							const logMin = Math.log10(dataMin);
							const logMax = Math.log10(dataMax);
							const logCenter = (logMin + logMax) / 2;
							const logHalfSpan = (logMax - logMin) / 2;

							const PADDING_DECADES = 0.5;
							const paddedHalfSpan = logHalfSpan + PADDING_DECADES;

							const finalLogMin = logCenter - paddedHalfSpan;
							const finalLogMax = logCenter + paddedHalfSpan;

							return [
								10 ** Math.floor(finalLogMin), 
								10 ** Math.ceil(finalLogMax)
							];
}
					};
				} else {
					scalesConfig[unit] = { auto: true };
				}
			}
		}

		const axesConfig: uPlot.Axis[] = [{}];

		if (this.viewType === 'fft') {
			scalesConfig['x'] = { time: false, distr: 3, log: 10 };
			axesConfig[0] = {
				scale: 'x',
				label: 'Frequency (Hz)'
			};

			for (const unit of uniqueUnits) {
				if (unit && scalesConfig[unit]) {
					scalesConfig[unit].distr = 3;
					scalesConfig[unit].log = 10;
				}
			}
		} else {
			scalesConfig['x'] = { time: false };
			axesConfig[0] = {
				scale: 'x',
				space: 100,
				values: (self, ticks) => {
					return ticks.map((rawTick) => {
						if (Math.abs(rawTick) < 1e-9) {
							return 'Now';
						}
						return `-${Math.abs(rawTick).toFixed(1)}s`;
					});
				}
			};
		}

		let yAxisCount = 0;

		for (const unit of uniqueUnits) {
			if (!unit) continue;

			const yAxisLabel = this.viewType === 'fft' ? `${unit}/√Hz` : unit;

			const axisOptions: uPlot.Axis = {
				scale: unit,
				label: yAxisLabel,
				labelGap: 5,
				stroke: this.series.find((s) => s.uPlotSeries.scale === unit)?.uPlotSeries.stroke,
				values: (u, vals) => {
					if (!vals.length) {
						return [];
					}

					return vals.map((v) => {
						if (v == null) return '';
						if (v === 0) return '0 ';

						const absV = Math.abs(v);

						if (absV > 0 && absV < 0.01) {
							return v.toExponential(1) + ' ';
						}
						if (absV < 10) {
							return v.toFixed(2) + ' ';
						}
						if (absV < 100) {
							return v.toFixed(1) + ' ';
						}
						return v.toFixed(0) + ' ';
					});
				}
			};

			if (yAxisCount > 0) {
				axisOptions.side = 1;
				axisOptions.grid = { show: false };
			}

			axesConfig.push(axisOptions);
			yAxisCount++;
		}

		const uplotSeriesConfig: uPlot.Series[] = [
			{}, // x-axis placeholder
			...this.series.map((s) => s.uPlotSeries)
		];

		return {
			width: 800,
			height: 400,
			series: uplotSeriesConfig,
			scales: scalesConfig,
			axes: axesConfig,
			pxAlign: 0,
			legend: { show: false },
			cursor: {
				drag: { setScale: false },
				show: true,
				points: { show: false },
				move: (u, top, left) => {
					return [top, left];
				}
			}
		};
	});

	constructor(
		title: string,
		initialSelection: RowSelectionState = {},
		initialExpansion: ExpandedState = {}
	) {
		this.title = title;
		this.rowSelection = initialSelection;
		this.expansion = initialExpansion;
	}
}

const DEFAULT_PLOT_HEIGHT = 400;

class ChartState {
	// --- CORE STATE ---
	plots = $state<PlotConfig[]>([]);
	layoutMode = $state<'auto' | 'manual'>('auto');
	containerHeight = $state(0);
	selectedPlotId = $state<string | null>(null);
	manualLayout = $state<Record<string, number>>({});
	isPaused = $state(false);

	layout = $derived.by(() => {
		if (this.layoutMode === 'manual') {
			return this.manualLayout;
		}

		// --- Auto Mode Logic ---
		const plots = this.plots;
		const containerHeight = this.containerHeight;
		const newLayout: Record<string, number> = {};

		if (plots.length === 0 || containerHeight === 0) {
			return {};
		}

		if (plots.length <= 4) {
			const heightPerPlot = containerHeight / plots.length;
			for (const plot of plots) {
				newLayout[plot.id] = heightPerPlot;
			}
		} else {
			const heightForFirstFour = containerHeight / 4;
			for (let i = 0; i < plots.length; i++) {
				newLayout[plots[i].id] = i < 4 ? heightForFirstFour : DEFAULT_PLOT_HEIGHT;
			}
		}
		return newLayout;
	});

	addPlot() {
		const initialExpansion: ExpandedState = {};
		for (const portData of deviceState.devices) {
			for (const device of portData.devices) {
				const deviceId = `${device.url}:${device.route}`;
				initialExpansion[deviceId] = true;
			}
		}
		const newPlot = new PlotConfig('New Plot', {}, initialExpansion);

		if (this.layoutMode === 'manual') {
			this.manualLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
		}
		this.plots.push(newPlot);
	}

	addPlotFromStream(dataKey: DataColumnId, streamName: string) {
		const selectionKey = JSON.stringify(dataKey);

		const deviceId = `${dataKey.port_url}:${dataKey.device_route}`;
		const streamId = `${deviceId}:${dataKey.stream_id}`;
		const initialExpansion: ExpandedState = {
			[deviceId]: true,
			[streamId]: true
		};

		const newPlot = new PlotConfig(
			`${streamName} Timeseries`,
			{ [selectionKey]: true },
			initialExpansion
		);

		if (this.layoutMode === 'manual') {
			this.manualLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
		}

		this.plots.push(newPlot);
	}

	removePlot(plotId: string) {
		if (this.plots.length === 1) {
			this.deleteAllPlots();
			return;
		}

		this.plots = this.plots.filter((p) => p.id !== plotId);
		delete this.manualLayout[plotId];
	}

	deleteAllPlots() {
		this.plots = [];
		this.manualLayout = {};
		this.layoutMode = 'auto';
	}

	rebalancePlots() {
		this.layoutMode = 'auto';
	}

	switchToManualMode() {
		if (this.layoutMode === 'manual') return;

		this.manualLayout = untrack(() => this.layout);
		this.layoutMode = 'manual';
	}

	updateLayoutFromManualResize(percentages: number[]) {
		const currentPlots = untrack(() => this.plots);
		if (this.layoutMode !== 'manual' || percentages.length !== currentPlots.length) return;

		const totalPixelHeight = Object.values(untrack(() => this.layout)).reduce((sum, h) => sum + h, 0);
		if (totalPixelHeight === 0) return;

		const newLayout: Record<string, number> = {};
		currentPlots.forEach((plot, i) => {
			newLayout[plot.id] = (totalPixelHeight * percentages[i]) / 100;
		});

		this.manualLayout = newLayout;
	}
	
	togglePause() {
		const newPausedState = !this.isPaused;
		this.isPaused = newPausedState;

		for (const plot of this.plots) {
			plot.isPaused = newPausedState;
		}
	}
}

export const chartState = new ChartState();