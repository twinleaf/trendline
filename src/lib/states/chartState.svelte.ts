import { deviceState } from '$lib/states/deviceState.svelte';
import { uiState } from '$lib/states/uiState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { DecimationMethod } from '$lib/bindings/DecimationMethod';
import type { DetrendMethod } from '$lib/bindings/DetrendMethod';
import type { RowSelectionState } from '@tanstack/table-core';
import type { ExpandedState } from '@tanstack/table-core';
import { untrack } from 'svelte';
import { Channel, invoke } from '@tauri-apps/api/core';
import type { PlotData } from '$lib/bindings/PlotData';
import { SvelteMap } from 'svelte/reactivity';
import type { SharedPlotConfig } from '$lib/bindings/SharedPlotConfig';

export type ChartLayout = 'carousel' | 'vertical' | 'horizontal';
export type StreamLayout = 'grouped' | 'vertical' | 'horizontal';

export interface PlotSeries {
    dataKey: DataColumnId;
    uPlotSeries: uPlot.Series;
}

/**
 * A utility class to assign and retrieve consistent colors for data columns
 * based on their unique key, ensuring a stable color palette across the application.
 */
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

	/**
	 * Gets a consistent style object for a given data column.
	 * If a style has not been assigned, it generates a new one.
	 * @param dataKey The unique identifier for the data column.
	 * @returns An object containing style properties (e.g., `{ color: string }`).
	 */
	getStyle(dataKey: DataColumnId): { color: string } {
		const key = JSON.stringify(dataKey);
		if (!this.#styles.has(key)) {
			this.#styles.set(key, { color: this.#getNextColor() });
		}
		return this.#styles.get(key)!;
	}
}


/**
 * Represents the complete state and configuration for a single plot in the chart area.
 * This includes data selection, view settings, and derived uPlot options.
 */
export class PlotConfig {
	/** A unique identifier for this plot instance, generated on creation. */
	id = crypto.randomUUID();
	/** The user-editable title of the plot. */
	title = $state('New Plot');
	/** State for selected rows (data columns) in the TanStack Table, keyed by a stringified `DataColumnId`. */
	rowSelection = $state<RowSelectionState>({});
	/** State for expanded rows (devices/streams) in the TanStack Table. */
	expansion = $state<ExpandedState>({});
	/** The currently active tab in the plot's settings header ('selection' or 'settings'). */
	activeTab = $state<'selection' | 'settings'>('selection');

	// --- Private state for view settings ---
	#manualDecimationMethod = $state<DecimationMethod>('Fpcs');
	#isDecimationManual = $state(false);

	/** The time window of data to display for timeseries view, in seconds. */
	windowSeconds = $state<number>(30.0);
	/** A multiplier affecting the data resolution for the timeseries view. */
	resolutionMultiplier = $state<number>(100);
	/** The time window of data to use for FFT calculation, in seconds. */
	fftSeconds = $state<number>(10.0);
	/** The detrending method to apply before performing an FFT. */
	fftDetrendMethod = $state<DetrendMethod>('None');
	/** A flag indicating if the plot has received any data from the backend. */
	hasData = $state(false);
	/** The most recent timestamp of data received by the plot, used for tracking data flow. */
	latestTimestamp = $state(0);
	/** A local pause state for this specific plot, which can override the global pause state. */
	isPaused = $state(false);
	/** The type of visualization to display ('timeseries' or 'fft'). */
	viewType = $state<'timeseries' | 'fft'>('timeseries');

	/**
	 * The data decimation method for the timeseries view.
	 * Defaults to 'Fpcs' unless a manual method is set by the user.
	 */
	get decimationMethod(): DecimationMethod {
		if (this.#isDecimationManual) {
			return this.#manualDecimationMethod;
		}
		return 'Fpcs';
	}
	set decimationMethod(value: DecimationMethod) {
		this.#isDecimationManual = true;
		this.#manualDecimationMethod = value;
	}

	/**
	 * A derived array of `PlotSeries` objects, generated from the `rowSelection` state.
	 * This connects the selected data columns to their uPlot display configuration.
	 */
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
							uPlotSeries: { label: column.name, stroke: style.color, scale: column.units }
						});
					}
				} catch (e) {
					console.error('Failed to parse a key that looked like JSON:', keyString, e);
				}
			}
		}
		return newSeries;
	});

	/**
	 * A derived value calculating the maximum effective sampling rate among all selected series in the plot.
	 * This is sent to the backend to configure the data processing pipeline correctly.
	 */
	maxSamplingRate = $derived.by((): number => {
		if (this.series.length === 0) return 0;
		const rates = this.series.map((s) => {
			const device = deviceState.getDevice(s.dataKey.port_url, s.dataKey.device_route);
			const stream = device?.streams.find((st) => st.meta.stream_id === s.dataKey.stream_id);
			return stream?.effective_sampling_rate ?? 0;
		});
		return Math.max(...rates);
	});

	/**
	 * A large derived object that generates the complete configuration for the `uPlot`
	 * charting library based on the current plot state (series, viewType, axes, etc.).
	 */
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
							if (dataMin <= 0 || !isFinite(dataMin)) return [1e-4, 1];
							const currentViewMin = u.scales[unit]?.min ?? null;
							const currentViewMax = u.scales[unit]?.max ?? null;
							let shouldResize = false;
							if (currentViewMin === null || currentViewMax === null) {
								shouldResize = true;
							} else {
								if (dataMin < currentViewMin || dataMax > currentViewMax) shouldResize = true;
								else {
									const viewDecades = Math.log10(currentViewMax) - Math.log10(currentViewMin);
									const dataDecades = Math.log10(dataMax) - Math.log10(dataMin);
									if (viewDecades > dataDecades + 2.0) shouldResize = true;
								}
							}
							if (!shouldResize) return [currentViewMin, currentViewMax];
							if (dataMin === dataMax) {
								const logVal = Math.log10(dataMin);
								return [10 ** (logVal - 1), 10 ** (logVal + 1)];
							}
							const logMin = Math.log10(dataMin);
							const logMax = Math.log10(dataMax);
							const logCenter = (logMin + logMax) / 2;
							const logHalfSpan = (logMax - logMin) / 2;
							const paddedHalfSpan = logHalfSpan + 0.5;
							return [
								10 ** Math.floor(logCenter - paddedHalfSpan),
								10 ** Math.ceil(logCenter + paddedHalfSpan)
							];
						}
					};
				} else scalesConfig[unit] = { auto: true };
			}
		}
		const axesConfig: uPlot.Axis[] = [{}];
		if (this.viewType === 'fft') {
			scalesConfig['x'] = { time: false, distr: 3, log: 10 };
			axesConfig[0] = { scale: 'x', label: 'Frequency (Hz)' };
			for (const unit of uniqueUnits)
				if (unit && scalesConfig[unit]) {
					scalesConfig[unit].distr = 3;
					scalesConfig[unit].log = 10;
				}
		} else {
			scalesConfig['x'] = { time: false };
			axesConfig[0] = {
				scale: 'x',
				space: 100,
				values: (_, ticks) =>
					ticks.map((raw) => (Math.abs(raw) < 1e-9 ? 'Now' : `-${Math.abs(raw).toFixed(1)}s`))
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
				values: (_, vals) =>
					vals.map((v) => {
						if (v == null) return '';
						if (v === 0) return '0 ';
						const absV = Math.abs(v);
						if (absV > 0 && absV < 0.01) return v.toExponential(1) + ' ';
						if (absV < 10) return v.toFixed(2) + ' ';
						if (absV < 100) return v.toFixed(1) + ' ';
						return v.toFixed(0) + ' ';
					})
			};
			if (yAxisCount > 0) {
				axisOptions.side = 1;
				axisOptions.grid = { show: false };
			}
			axesConfig.push(axisOptions);
			yAxisCount++;
		}
		const uplotSeriesConfig: uPlot.Series[] = [{}, ...this.series.map((s) => s.uPlotSeries)];
		return {
			width: 800,
			height: 400,
			series: uplotSeriesConfig,
			scales: scalesConfig,
			axes: axesConfig,
			pxAlign: 0,
			legend: { show: false },
			cursor: {
				drag: {
					setScale: false
				},
				show: true,
				points: { show: false },
				move: (_, t, l) => [t, l],
				bind: {
					dblclick: (u) => {
						return null;
					}
				}
			}
		};
	});

	/**
	 * Creates a new PlotConfig instance.
	 * @param title The initial title for the plot.
	 * @param sel The initial row selection state.
	 * @param exp The initial row expansion state.
	 * @param paused The initial pause state.
	 */
	constructor(
        title: string,
        sel: RowSelectionState = {},
        exp: ExpandedState = {},
        paused = false
    ) {
		this.title = title;
		this.rowSelection = sel;
		this.expansion = exp;
        this.isPaused = paused;
	}
}

const DEFAULT_PLOT_HEIGHT = 400;

class ChartState {
	// --- Public State ---

	/** An array of PlotConfig objects, representing all charts currently displayed. */
	plots = $state<PlotConfig[]>([]);
	/** A map from a plot's unique ID to its latest stream of PlotData. */
	plotsData = new SvelteMap<string, PlotData>();
	/** A global flag to pause or resume data updates for all plots. */
	isPaused = $state(false);

	// --- Layout State ---

	/** The current layout mode. 'auto' distributes plots automatically, 'manual' allows user-defined sizes. */
	layoutMode = $state<'auto' | 'manual'>('auto');
	/** The height of the main chart area container in pixels, measured by a ResizeObserver. */
	containerHeight = $state(0);
	/** A record of plot IDs to their user-defined heights in pixels, used only in 'manual' mode. */
	manualLayout = $state<Record<string, number>>({});

	// --- Private State ---

	/** A set of plot IDs that have an active data listener on the backend. */
	#listeningPlots = new Set<string>();
	/** A flag used to lock actions and prevent rapid-fire calls from double-clicks. */
	#isActionLocked = false;
	/** The timeout ID for the action lock, used to release the lock after a cooldown. */
	#actionLockTimeout: number | undefined;

	// --- Private Helper for Rate Limiting ---
	/**
	 * A helper to prevent rapid execution of state-mutating UI actions like adding or moving plots.
	 * It executes the action immediately, then enforces a short cooldown to prevent double-clicks.
	 * @param actionFn The function to execute.
	 * @param cooldown The cooldown period in milliseconds.
	 */
	#withActionLock(actionFn: () => void, cooldown = 200) {
		if (this.#isActionLocked) {
			return;
		}
		this.#isActionLocked = true;

		try {
			actionFn();
		} finally {
			this.#actionLockTimeout = window.setTimeout(() => {
				this.#isActionLocked = false;
			}, cooldown);
		}
	}

	// --- CORE LAYOUT LOGIC ---

	/**
	 * A derived property that calculates the layout of plots.
	 * In 'manual' mode, it returns the user-defined layout.
	 * In 'auto' mode, it fits 1-4 plots within the viewport and uses a fixed height for plots beyond four, creating a scrollbar.
	 */
	layout = $derived.by(() => {
		if (this.layoutMode === 'manual') {
			return this.manualLayout;
		}

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

	/**
	 * Checks if a specific plot has any data associated with it.
	 * This is used to disable UI elements proactively.
	 * @param plotId The ID of the plot to check.
	 * @returns `true` if the plot has data, `false` otherwise.
	 */
	plotHasData(plotId: string): boolean {
		const data = this.plotsData.get(plotId);
		return data ? data.timestamps.length > 0 : false;
	}

	/**
	 * Cleans up all plots and backend resources. Should be called when the component is destroyed.
	 */
	destroy() {
		this.deleteAllPlots();
	}

	/**
	 * Synchronizes the state of a single plot with the backend, creating or updating its data processing pipeline.
	 * This is a declarative "upsert" operation.
	 * @param plot The PlotConfig object to sync.
	 */
	async syncPlotWithBackend(plot: PlotConfig) {
		untrack(async () => {
			if (plot.series.length === 0) {
				if (this.#listeningPlots.has(plot.id)) {
					await this.destroyPlotOnBackend(plot.id);
				}
				return;
			}

			if (!this.#listeningPlots.has(plot.id)) {
				const plotChannel = new Channel<PlotData>();
				plotChannel.onmessage = (data) => {
					if (!this.isPaused && !plot.isPaused) {
						this.plotsData.set(plot.id, data);
					}
				};
				try {
					await invoke('listen_to_plot_data', {
						plotId: plot.id,
						onEvent: plotChannel
					});
					this.#listeningPlots.add(plot.id);
				} catch (e) {
					console.error(`[IPC] Failed to invoke listener for plot ${plot.id}`, e);
				}
			}

			const viewConfig =
				plot.viewType === 'timeseries'
					? {
							Timeseries: {
								decimation_method: plot.decimationMethod,
								window_seconds: plot.windowSeconds,
								resolution_multiplier: plot.resolutionMultiplier
							}
						}
					: {
							Fft: {
								window_seconds: plot.fftSeconds,
								detrend_method: plot.fftDetrendMethod
							}
						};

			const configForBackend: SharedPlotConfig = {
				plot_id: plot.id,
				data_keys: plot.series.map((s) => s.dataKey),
				max_sampling_rate: plot.maxSamplingRate,
				view_config: viewConfig
			};

			try {
				await invoke('update_plot_pipeline', { config: configForBackend });
			} catch (e) {
				console.error(`[Frontend] Failed to sync pipeline for plot ${plot.id}:`, e);
			}
		});
	}

	/**
	 * Tears down the data pipeline for a specific plot on the backend.
	 * @param plotId The ID of the plot to destroy on the backend.
	 */
	async destroyPlotOnBackend(plotId: string) {
		if (this.#listeningPlots.has(plotId)) {
			this.#listeningPlots.delete(plotId);
		}
		try {
			await invoke('destroy_plot_pipeline', { plotId });
		} catch (e) {
			console.error(`[Frontend] Failed to destroy backend pipeline for plot ${plotId}:`, e);
		}
	}

	// --- Public Methods ---

	/**
	 * Gets the array index of a plot by its ID.
	 * @param plotId The ID of the plot.
	 * @returns The index of the plot, or -1 if not found.
	 */
	getPlotIndex(plotId: string): number {
		return this.plots.findIndex((p) => p.id === plotId);
	}

	/**
	 * Adds a new, empty plot to the end of the chart list.
	 */
	addPlot() {
		this.#withActionLock(() => {
			const initialExpansion: ExpandedState = {};
			for (const portData of deviceState.devices) {
				for (const device of portData.devices) initialExpansion[`${device.url}:${device.route}`] = true;
			}
			const newPlot = new PlotConfig('New Plot', {}, initialExpansion, this.isPaused);
			this.plots.push(newPlot);
			if (this.layoutMode === 'manual') {
				const newLayout = { ...this.manualLayout };
				newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
				this.manualLayout = newLayout;
			}
		});
	}

	/**
	 * Adds a new, empty plot immediately above a specified plot.
	 * @param plotId The ID of the plot to add the new plot above.
	 */
	addPlotAbove(plotId: string) {
		this.#withActionLock(() => {
			const index = this.getPlotIndex(plotId);
			if (index === -1) return;
			const initialExpansion: ExpandedState = {};
			for (const portData of deviceState.devices) {
				for (const device of portData.devices) initialExpansion[`${device.url}:${device.route}`] = true;
			}
			const newPlot = new PlotConfig('New Plot', {}, initialExpansion, this.isPaused);
			this.plots.splice(index, 0, newPlot);
			if (this.layoutMode === 'manual') {
				const newLayout = { ...this.manualLayout };
				newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
				this.manualLayout = newLayout;
			}
		});
	}

	/**
	 * Adds a new, empty plot immediately below a specified plot.
	 * @param plotId The ID of the plot to add the new plot below.
	 */
	addPlotBelow(plotId: string) {
		this.#withActionLock(() => {
			const index = this.getPlotIndex(plotId);
			if (index === -1) return;
			const initialExpansion: ExpandedState = {};
			for (const portData of deviceState.devices) {
				for (const device of portData.devices) initialExpansion[`${device.url}:${device.route}`] = true;
			}
			const newPlot = new PlotConfig('New Plot', {}, initialExpansion, this.isPaused);
			this.plots.splice(index + 1, 0, newPlot);
			if (this.layoutMode === 'manual') {
				const newLayout = { ...this.manualLayout };
				newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
				this.manualLayout = newLayout;
			}
		});
	}

	/**
	 * Adds a new plot that is pre-configured with a selected data column.
	 * @param dataKey The data column identifier to pre-select.
	 * @param streamName The name of the stream for the plot title.
	 */
	addPlotFromStream(dataKey: DataColumnId, streamName: string) {
		this.#withActionLock(() => {
			const selectionKey = JSON.stringify(dataKey);
			const deviceId = `${dataKey.port_url}:${dataKey.device_route}`;
			const streamId = `${deviceId}:${dataKey.stream_id}`;
			const initialExpansion: ExpandedState = { [deviceId]: true, [streamId]: true };
			const newPlot = new PlotConfig(`${streamName} Timeseries`, { [selectionKey]: true }, initialExpansion, this.isPaused);
			this.plots.push(newPlot);
			if (this.layoutMode === 'manual') {
				const newLayout = { ...this.manualLayout };
				newLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
				this.manualLayout = newLayout;
			}
		});
	}

	/**
	 * Moves a plot up or down in the list.
	 * @param plotId The ID of the plot to move.
	 * @param direction The direction to move the plot, either 'up' or 'down'.
	 */
	movePlot(plotId: string, direction: 'up' | 'down') {
		this.#withActionLock(() => {
			const index = this.getPlotIndex(plotId);
			if (index === -1) return;
			const newIndex = direction === 'up' ? index - 1 : index + 1;
			if (newIndex < 0 || newIndex >= this.plots.length) return;
			const newPlots = [...this.plots];
			const [movedPlot] = newPlots.splice(index, 1);
			newPlots.splice(newIndex, 0, movedPlot);
			this.plots = newPlots;
		});
	}

	/**
	 * Copies the decimated data currently visible in the plot to the clipboard.
	 * @param plotId The ID of the plot to move.
	 */
	async copyPlotViewToClipboard(plotId: string) {
		const plot = this.plots.find((p) => p.id === plotId);
		if (!plot) { uiState.showError('Plot not found.'); return; }

		const decimatedData = this.plotsData.get(plotId);
		if (!decimatedData || decimatedData.timestamps.length === 0) {
			uiState.showError('No plot data available to copy.');
			return;
		}

		try {
			await invoke('export_plot_data_to_clipboard', {
				plotData: decimatedData,
				dataColumnIds: plot.series.map(s => s.dataKey)
			});
		} catch (e) {
			uiState.showError(e as string);
		}
	}

	/**
	 * Saves the decimated data currently visible in the plot to a CSV file.
	 * @param plotId The ID of the plot whose data will be saved.
	 */
	async savePlotDataAsCsv(plotId: string) {
		const plot = this.plots.find((p) => p.id === plotId);
		if (!plot) {
			uiState.showError('Plot not found.');
			return;
		}

		const decimatedData = this.plotsData.get(plotId);
		if (!decimatedData || decimatedData.timestamps.length === 0) {
			uiState.showError('No plot data available to save.');
			return;
		}

		try {
			await invoke('save_plot_data_to_file', {
				plotData: decimatedData,
				dataColumnIds: plot.series.map((s) => s.dataKey),
				fileNameSuggestion: `${plot.title.replace(/\s+/g, '_')}_plotted.csv`
			});
		} catch (e) {
			uiState.showError(e as string);
		}
	}

	/**
	 * Saves the high-resolution raw data corresponding to the plot's current view.
	 * It intelligently uses a pre-existing snapshot if the plot is paused,
	 * or performs an instant "live snapshot" if the plot is running.
	 * @param plotId The ID of the plot to move.
	 */
	async saveRawDataAsCsv(plotId: string) {
		const plot = this.plots.find((p) => p.id === plotId);
		if (!plot) { 
			uiState.showError('Plot not found.'); 
			return; 
		}

		const viewData = this.plotsData.get(plotId);
		if (!viewData || viewData.timestamps.length === 0) {
			uiState.showError('No data in view to define a time range for saving.');
			return;
		}
		
		const startTime = Math.min(...viewData.timestamps);
		const endTime = Math.max(...viewData.timestamps);

		try {
			uiState.showInfo("Fetching and preparing raw data...");
			await invoke('save_raw_plot_data_to_file', {
				plotId: plot.id,
				dataColumnIds: plot.series.map((s) => s.dataKey),
				startTime,
				endTime,
				isPaused: plot.isPaused,
				fileNameSuggestion: `${plot.title.replace(/\s+/g, '_')}_raw.csv`
			});
		} catch (e) {
			uiState.showError(e as string);
		}
	}

	/**
	 * Removes a plot from the chart area and intelligently updates the layout.
	 * If in 'manual' mode, it rescales the remaining plots to fill the viewport.
	 * @param plotId The ID of the plot to remove.
	 */
	removePlot(plotId: string) {
		const plotIndex = this.plots.findIndex((p) => p.id === plotId);
		if (plotIndex === -1) return;

		const oldManualLayout = { ...this.manualLayout };
		const wasInManualMode = this.layoutMode === 'manual';

		invoke('unpause_plot', { plotId }).catch(e => console.error(`Failed to clear snapshot for removed plot ${plotId}:`, e));

		this.destroyPlotOnBackend(plotId);
		this.plots.splice(plotIndex, 1);
		this.plotsData.delete(plotId);

		if (wasInManualMode) {
			if (this.plots.length <= 1) {
				this.rebalancePlots();
				return;
			}
			const newLayout: Record<string, number> = {};
			const remainingSum = this.plots.reduce((sum, p) => sum + (oldManualLayout[p.id] || 0), 0);
			if (remainingSum > 0 && this.containerHeight > 0) {
				const scaleFactor = this.containerHeight / remainingSum;
				for (const p of this.plots) {
					const oldHeight = oldManualLayout[p.id] || 0;
					newLayout[p.id] = oldHeight * scaleFactor;
				}
				this.manualLayout = newLayout;
			} else {
				this.rebalancePlots();
			}
		}
	}

	/**
	 * Removes all plots from the chart area and resets the layout state.
	 */
	deleteAllPlots() {
		untrack(() => {
			for (const plot of this.plots) {
				this.destroyPlotOnBackend(plot.id);
			}
		});
		this.plots = [];
		this.manualLayout = {};
		this.plotsData.clear();
		this.layoutMode = 'auto';
	}

	/**
	 * Resets the plot layout to 'auto' mode, causing all plots to be resized to fit the viewport.
	 */
	rebalancePlots() {
		this.layoutMode = 'auto';
	}

	/**
	 * Sets a plot's height to a specific percentage of the viewport height.
	 * This is a direct, imperative command that does not affect the size of other plots.
	 * @param plotId The ID of the plot to resize.
	 * @param viewPercentage The desired height as a percentage of the viewport (e.g., 50).
	 */
	setPlotHeight(plotId: string, viewPercentage: number) {
		this.#withActionLock(() => {
			this.switchToManualMode();
			if (this.containerHeight <= 0) return;
			const newLayout = { ...this.manualLayout };
			const targetHeight = this.containerHeight * (viewPercentage / 100);
			newLayout[plotId] = targetHeight;
			this.manualLayout = newLayout;
		});
	}

	/**
	 * Compares two numbers with a small tolerance.
	 * @private
	 */
	_isClose(a: number, b: number, epsilon = 0.1): boolean {
		return Math.abs(a - b) < epsilon;
	}

	/**
	 * Gets the preset percentage key for a plot's height, used for UI checkmarks.
	 * @param plotId The ID of the plot to check.
	 * @returns A string key ('25', '33', '50', '100') or undefined if its height doesn't match a preset.
	 */
	getPlotHeightPercentageKey(plotId: string): string | undefined {
		const plotHeight = this.layout[plotId];
		if (!plotHeight || this.containerHeight <= 0) {
			return undefined;
		}
		const percentage = (plotHeight / this.containerHeight) * 100;
		if (this._isClose(percentage, 100)) return '100';
		if (this._isClose(percentage, 50)) return '50';
		if (this._isClose(percentage, 33.33)) return '33';
		if (this._isClose(percentage, 25)) return '25';
		return undefined;
	}

	/**
	 * Gets the current type of plot. Used for validating context menu options.
	 * @param plotId The ID of the plot to check.
	 * @returns A string key ('timeseries', 'fft') or undefined if the plot is not found.
	 */
	getPlotType(plotId: string): 'timeseries' | 'fft' | undefined {
		const plot = this.plots.find((p) => p.id === plotId);
		return plot?.viewType;
	}

	/**
	 * Switches the layout to 'manual' mode, taking a snapshot of the current layout.
	 */
	switchToManualMode() {
		if (this.layoutMode === 'manual') return;
		this.manualLayout = untrack(() => this.layout);
		this.layoutMode = 'manual';
	}

	/**
	 * Updates the manual layout based on percentage values from the resizable pane component.
	 * @param percentages An array of numbers (0-100) representing the new size of each pane.
	 */
	updateLayoutFromManualResize(percentages: number[]) {
		const currentPlots = untrack(() => this.plots);
		if (this.layoutMode !== 'manual' || percentages.length !== currentPlots.length) return;
		const totalPixelHeight = Object.values(untrack(() => this.layout)).reduce(
			(sum, h) => sum + h,
			0
		);
		if (totalPixelHeight === 0) return;
		const newLayout: Record<string, number> = {};
		currentPlots.forEach((plot, i) => {
			newLayout[plot.id] = (totalPixelHeight * percentages[i]) / 100;
		});
		this.manualLayout = newLayout;
	}

	/**
	 * Toggles the pause state for a SINGLE plot and orchestrates the
	 * creation or deletion of its backend snapshot.
	 */
	async togglePlotPause(plot: PlotConfig) {
		const intendedNewState = !plot.isPaused;

		if (!intendedNewState && this.isPaused) {
			uiState.showInfo("Cannot un-pause an individual plot while global pause is active.");
			return;
		}

		try {
			if (intendedNewState) {
				const viewData = this.plotsData.get(plot.id);
				if (!viewData || viewData.timestamps.length === 0) {
					uiState.showWarning(`Cannot pause "${plot.title}": plot has no data.`);
					return;
				}
				const startTime = Math.min(...viewData.timestamps);
				const endTime = Math.max(...viewData.timestamps);
				
				await invoke('pause_plot', { plotId: plot.id, startTime, endTime });
			} else {
				await invoke('unpause_plot', { plotId: plot.id });
			}
			plot.isPaused = intendedNewState;
		} catch (e) {
			uiState.showError(e as string);
		}
	}

	/**
	 * Toggles the GLOBAL pause state for all plots.
	 * This is what the Spacebar hotkey will call.
	 */
	toggleGlobalPause() {
		const newGlobalPausedState = !this.isPaused;
		this.isPaused = newGlobalPausedState;
		for (const plot of this.plots) {
			this.togglePlotPause(plot);
		}
	}
}

/** A singleton instance of the DataColumnStyler for global use. */
export const dataColumnStyler = new DataColumnStyler();
/** A global class state for manipulating ChartView plots and their configuration. */
export const chartState = new ChartState();