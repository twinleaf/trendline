import { deviceState } from '$lib/states/deviceState.svelte';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { DecimationMethod } from '$lib/bindings/DecimationMethod';
import type { DetrendMethod } from '$lib/bindings/DetrendMethod';
import type { RowSelectionState } from '@tanstack/table-core';
import type { ExpandedState } from '@tanstack/table-core';
import { untrack } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import type { PipelineId } from '$lib/bindings/PipelineId';
import type { PlotData } from '$lib/bindings/PlotData';
import { SvelteMap } from 'svelte/reactivity';

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

    // Updated pipelineMap to track the full chain for FFTs
    pipelineMap = new SvelteMap<string, { 
        timeseriesId: PipelineId | null; 
        fftSourceId: PipelineId | null; // Intermediate detrend pipeline
        fftId: PipelineId | null; 
    }>();

    #manualDecimationMethod = $state<DecimationMethod>('Fpcs');
    #isDecimationManual = $state(false);
    windowSeconds = $state<number>(30.0);
    resolutionMultiplier = $state<number>(100);
    fftSeconds = $state<number>(10.0);
    fftDetrendMethod = $state<DetrendMethod>('None');
    hasData = $state(false);
    latestTimestamp = $state(0);
    isPaused = $state(false);
    viewType = $state<'timeseries' | 'fft'>('timeseries');

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

    maxSamplingRate = $derived.by((): number => {
        if (this.series.length === 0) return 0;
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
                width: 800, height: 400, legend: { show: false }, series: [{}], axes: [{}, { show: false }]
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
                            return [10 ** Math.floor(logCenter - paddedHalfSpan), 10 ** Math.ceil(logCenter + paddedHalfSpan)];
                        }
                    };
                } else scalesConfig[unit] = { auto: true };
            }
        }
        const axesConfig: uPlot.Axis[] = [{}];
        if (this.viewType === 'fft') {
            scalesConfig['x'] = { time: false, distr: 3, log: 10 };
            axesConfig[0] = { scale: 'x', label: 'Frequency (Hz)' };
            for (const unit of uniqueUnits) if (unit && scalesConfig[unit]) {
                scalesConfig[unit].distr = 3;
                scalesConfig[unit].log = 10;
            }
        } else {
            scalesConfig['x'] = { time: false };
            axesConfig[0] = { scale: 'x', space: 100, values: (_, ticks) => ticks.map((raw) => Math.abs(raw) < 1e-9 ? 'Now' : `-${Math.abs(raw).toFixed(1)}s`) };
        }
        let yAxisCount = 0;
        for (const unit of uniqueUnits) {
            if (!unit) continue;
            const yAxisLabel = this.viewType === 'fft' ? `${unit}/√Hz` : unit;
            const axisOptions: uPlot.Axis = {
                scale: unit, label: yAxisLabel, labelGap: 5, stroke: this.series.find((s) => s.uPlotSeries.scale === unit)?.uPlotSeries.stroke,
                values: (_, vals) => vals.map((v) => {
                    if (v == null) return ''; if (v === 0) return '0 '; const absV = Math.abs(v);
                    if (absV > 0 && absV < 0.01) return v.toExponential(1) + ' '; if (absV < 10) return v.toFixed(2) + ' ';
                    if (absV < 100) return v.toFixed(1) + ' '; return v.toFixed(0) + ' ';
                })
            };
            if (yAxisCount > 0) { axisOptions.side = 1; axisOptions.grid = { show: false }; }
            axesConfig.push(axisOptions); yAxisCount++;
        }
        const uplotSeriesConfig: uPlot.Series[] = [{}, ...this.series.map((s) => s.uPlotSeries)];
        return {
            width: 800, height: 400, series: uplotSeriesConfig, scales: scalesConfig, axes: axesConfig, pxAlign: 0,
            legend: { show: false }, cursor: { drag: { setScale: false }, show: true, points: { show: false }, move: (_, t, l) => [t, l] }
        };
    });

    constructor(title: string, sel: RowSelectionState = {}, exp: ExpandedState = {}) {
        this.title = title;
        this.rowSelection = sel;
        this.expansion = exp;
    }
}

const DEFAULT_PLOT_HEIGHT = 400;

class ChartState {
    // --- Public State ---
    plots = $state<PlotConfig[]>([]);
    plotsData = new SvelteMap<string, PlotData>();
    isPaused = $state(false);

    // --- Layout State ---
    layoutMode = $state<'auto' | 'manual'>('auto');
    containerHeight = $state(0);
    manualLayout = $state<Record<string, number>>({});

    // --- Private State ---
    #pollingIntervalId: ReturnType<typeof setInterval> | null = null;

    // --- Layout Logic ---
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

    // --- "Command" Methods to be called by components ---

    initPolling() {
		if (this.#pollingIntervalId) return;
		let isFetching = false;

		this.#pollingIntervalId = setInterval(async () => {
			if (this.isPaused || this.plots.length === 0 || isFetching) return;
			
			isFetching = true;
			try {
				const fetchPromises = this.plots.map(async (plot) => {
					if (plot.isPaused || plot.series.length === 0) return;

					const pipelineIds = Array.from(plot.pipelineMap.values())
						.map((p) => (plot.viewType === 'fft' ? p.fftId : p.timeseriesId))
						.filter((id): id is PipelineId => !!id);
					
					if (pipelineIds.length > 0) {
						try {
							const data = await invoke<PlotData>('get_merged_plot_data', { ids: pipelineIds });
							this.plotsData.set(plot.id, data);
						} catch (e) {

						}
					}
				});
				await Promise.all(fetchPromises);
			} finally {
				isFetching = false;
			}
		}, 33);
	}

    destroy() {
        if (this.#pollingIntervalId) {
            clearInterval(this.#pollingIntervalId);
            this.#pollingIntervalId = null;
        }
        untrack(() => {
            for (const plot of this.plots) this._destroyPipelinesForPlot(plot);
        });
    }

    updateAllPlotPipelines() {
        for (const plot of this.plots) {
            const currentSeriesKeys = new Set(plot.series.map((s) => JSON.stringify(s.dataKey)));
            const managedKeys = new Set(plot.pipelineMap.keys());
            const keysToAdd = [...currentSeriesKeys].filter((k) => !managedKeys.has(k));
            const keysToRemove = [...managedKeys].filter((k) => !currentSeriesKeys.has(k));
            const keysToUpdate = [...managedKeys].filter((k) => currentSeriesKeys.has(k));

            for (const keyStr of keysToAdd) {
                plot.pipelineMap.set(keyStr, { timeseriesId: null, fftSourceId: null, fftId: null });
                this._createTimeseriesPipeline(plot, keyStr);
            }
            for (const keyStr of keysToRemove) this._destroyPipelinesForKey(plot, keyStr);
            
            for (const keyStr of keysToUpdate) {
                const pipelines = plot.pipelineMap.get(keyStr);
                if (!pipelines) continue;

                if (plot.viewType === 'fft') {
                    if (!pipelines.fftId) {
                        this._createFftChain(plot, keyStr);
                    }
                    // NOTE: This doesn't handle changes to FFT settings (e.g. detrend method) yet.
                    // That would require a more complex reconciliation.
                } else { // timeseries view
                    if (pipelines.fftId) {
                        this._destroyFftChain(plot, keyStr);
                    }
                    // NOTE: This doesn't handle changes to timeseries settings (e.g. decimation method).
                }
            }
        }
    }

    // --- Private Helpers ---

    _createTimeseriesPipeline(plot: PlotConfig, keyStr: string) {
        try {
            const dataKey = JSON.parse(keyStr);
            let promise: Promise<PipelineId>;

            if (plot.decimationMethod === 'Fpcs') {
                const totalPointsInWindow = plot.maxSamplingRate * plot.windowSeconds;
                const targetPoints = 10 * plot.resolutionMultiplier; // Density factor
                let ratio = Math.round(totalPointsInWindow / targetPoints);
                if (ratio < 1) ratio = 1;

                console.log(`[Frontend] Creating FPCS pipeline for ${keyStr} with ratio ${ratio}`);
                promise = invoke('create_fpcs_pipeline', { 
                    sourceKey: dataKey, 
                    ratio, 
                    windowSeconds: plot.windowSeconds // Add this line
                });

            } else { // 'None'
                console.log(`[Frontend] Creating Passthrough pipeline for ${keyStr}`);
                promise = invoke('create_passthrough_pipeline', { sourceKey: dataKey, windowSeconds: plot.windowSeconds });
            }

            promise.then((id) => {
                console.log(`[Frontend] Timeseries pipeline created on backend. ID:`, id);
                const entry = plot.pipelineMap.get(keyStr);
                if (entry) entry.timeseriesId = id;
            }).catch((e) => console.error(`Failed to create timeseries pipeline for ${keyStr}:`, e));

        } catch (e) { console.error(`Error parsing key to create pipeline: ${keyStr}`, e); }
    }

    async _createFftChain(plot: PlotConfig, keyStr: string) {
        const pipelines = plot.pipelineMap.get(keyStr);
        if (!pipelines || !pipelines.timeseriesId) return;

        try {
            const dataKey = JSON.parse(keyStr);
            let fftSourceId = pipelines.timeseriesId;
            let intermediateId: PipelineId | null = null;

            // If detrending is needed, create a new detrend pipeline as the source for the FFT.
            if (plot.fftDetrendMethod !== 'None') {
                console.log(`[Frontend] Creating intermediate Detrend pipeline for FFT.`);
                const detrendId = await invoke<PipelineId>('create_detrend_pipeline', {
                    sourceKey: dataKey,
                    windowSeconds: plot.fftSeconds,
                    method: plot.fftDetrendMethod,
                });
                console.log(`[Frontend] Detrend pipeline created. ID:`, detrendId);
                fftSourceId = detrendId;
                intermediateId = detrendId;
            }
            
            console.log(`[Frontend] Creating FFT pipeline from source: ${fftSourceId}`);
            const fftId = await invoke<PipelineId>('create_fft_pipeline_from_source', { 
                sourcePipelineId: fftSourceId 
            });
            console.log(`[Frontend] FFT pipeline created. ID: ${fftId}`);

            const entry = plot.pipelineMap.get(keyStr);
            if (entry) {
                entry.fftSourceId = intermediateId;
                entry.fftId = fftId;
            }

        } catch (e) {
            console.error(`Failed to create FFT chain for ${keyStr}:`, e);
        }
    }
    
    _destroyFftChain(plot: PlotConfig, keyStr: string) {
        const pipelines = plot.pipelineMap.get(keyStr);
        if (!pipelines) return;

        if (pipelines.fftId) {
            console.log(`[Frontend] Destroying FFT pipeline on backend. ID:`, pipelines.fftId);
            invoke('destroy_processor', { id: pipelines.fftId });
            pipelines.fftId = null;
        }
        if (pipelines.fftSourceId) {
            console.log(`[Frontend] Destroying intermediate FFT source pipeline. ID:`, pipelines.fftSourceId);
            invoke('destroy_processor', { id: pipelines.fftSourceId });
            pipelines.fftSourceId = null;
        }
    }

    _destroyPipelinesForKey(plot: PlotConfig, keyStr: string) {
        const pipelines = plot.pipelineMap.get(keyStr);
        if (pipelines) {
            if (pipelines.timeseriesId) {
                console.log(`[Frontend] Destroying timeseries pipeline on backend. ID:`, pipelines.timeseriesId);
                invoke('destroy_processor', { id: pipelines.timeseriesId });
            }
            this._destroyFftChain(plot, keyStr); // Also destroy any FFT components
        }
        plot.pipelineMap.delete(keyStr);
    }

    _destroyPipelinesForPlot(plot: PlotConfig) {
        for (const keyStr of plot.pipelineMap.keys()) {
            this._destroyPipelinesForKey(plot, keyStr);
        }
    }
    
    // --- Public Methods ---

    addPlot() {
        const initialExpansion: ExpandedState = {};
        for (const portData of deviceState.devices) {
            for (const device of portData.devices) initialExpansion[`${device.url}:${device.route}`] = true;
        }
        const newPlot = new PlotConfig('New Plot', {}, initialExpansion);
        if (this.layoutMode === 'manual') this.manualLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
        this.plots.push(newPlot);
    }

    addPlotFromStream(dataKey: DataColumnId, streamName: string) {
        const selectionKey = JSON.stringify(dataKey);
        const deviceId = `${dataKey.port_url}:${dataKey.device_route}`;
        const streamId = `${deviceId}:${dataKey.stream_id}`;
        const initialExpansion: ExpandedState = { [deviceId]: true, [streamId]: true };
        const newPlot = new PlotConfig(`${streamName} Timeseries`, { [selectionKey]: true }, initialExpansion);
        if (this.layoutMode === 'manual') this.manualLayout[newPlot.id] = DEFAULT_PLOT_HEIGHT;
        this.plots.push(newPlot);
    }

    removePlot(plotId: string) {
        const plotIndex = this.plots.findIndex((p) => p.id === plotId);
        if (plotIndex === -1) return;
        const plotToRemove = this.plots[plotIndex];
        this._destroyPipelinesForPlot(plotToRemove);
        this.plots.splice(plotIndex, 1);
        this.plotsData.delete(plotId);
        delete this.manualLayout[plotId];
        if (this.plots.length === 0) this.layoutMode = 'auto';
    }

    deleteAllPlots() {
        untrack(() => {
            for (const plot of this.plots) this._destroyPipelinesForPlot(plot);
        });
        this.plots = [];
        this.manualLayout = {};
        this.plotsData.clear();
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
