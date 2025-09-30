<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { chartState } from '$lib/states/chartState.svelte';
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import CustomLegend from '$lib/components/chart-area/legend/CustomLegend.svelte';
	import { findTimestampIndex, lerp } from '$lib/utils';

	// --- Props ---
	let { plot, latestTimestamp = $bindable() }: { plot: PlotConfig; latestTimestamp?: number } = $props();

	// --- Component State ---
	let chartContainer: HTMLDivElement;
	let uplot = $state.raw<uPlot | undefined>(undefined);
	let legendState = $state({
		isActive: false,
		relativeTime: null as number | null,
		frequency: null as number | null,
		values: [] as (number | null)[],
		cursorLeft: null as number | null,
		cursorTop: null as number | null,
		chartBounds: null as DOMRect | null
	});

	// --- Primary Derived State (from props and global state) ---
	const options = $derived(plot.uPlotOptions);
	const isFFT = $derived(plot.viewType === 'fft');
	const isEffectivelyPaused = $derived(plot.isPaused);
	const rawPlotData = $derived(chartState.plotsData.get(plot.id));

	// --- Buffer Management ---
	let uplotDataBuffers = $state.raw<Float64Array[]>([]);
	$effect(() => {
		const numSeries = plot.series.length;
		const maxRate = plot.maxSamplingRate;
		const winSecs = plot.windowSeconds;

		if (numSeries === 0 || maxRate <= 0 || winSecs <= 0) {
			if (uplotDataBuffers.length > 0) uplotDataBuffers = [];
			return;
		}

		const requiredPoints = Math.ceil(maxRate * winSecs * 2);
		const needsReallocation =
			uplotDataBuffers.length !== numSeries + 1 ||
			(uplotDataBuffers.length > 0 && uplotDataBuffers[0].length < requiredPoints);

		if (needsReallocation) {
			console.log(`[Buffer] Reallocating for ${numSeries} series.`);
			const newBuffers = [new Float64Array(requiredPoints)];
			for (let i = 0; i < numSeries; i++) newBuffers.push(new Float64Array(requiredPoints));
			uplotDataBuffers = newBuffers;
		}
	});

	// --- Data Preparation Logic ---
	const preparedData = $derived.by((): { views: Float64Array[]; latestTimestamp: number } | null => {
		const dataToRender = rawPlotData;
		const numSeries = plot.series.length;

		if (!dataToRender || dataToRender.timestamps.length === 0 || numSeries === 0) return null;

		const isFFTView = isFFT;
		let finalTimestamps = dataToRender.timestamps;
		let finalSeriesData = dataToRender.series_data;

		if (isFFTView && finalTimestamps.length > 1) {
			finalTimestamps = finalTimestamps.slice(1);
			finalSeriesData = finalSeriesData.map((series) => series.slice(1));
		}

		const finalDataLength = finalTimestamps.length;
		if (finalDataLength === 0) return null;

		if (uplotDataBuffers.length !== numSeries + 1 || uplotDataBuffers[0].length < finalDataLength) return null;

		const latestAbsTimestamp = dataToRender.timestamps[dataToRender.timestamps.length - 1];

		if (!isFFTView) {
			const relativeXValues = uplotDataBuffers[0];
			for (let i = 0; i < finalDataLength; i++) {
				relativeXValues[i] = finalTimestamps[i] - latestAbsTimestamp;
			}
		} else {
			uplotDataBuffers[0].set(finalTimestamps);
		}

		finalSeriesData.forEach((series, i) => {
			const buffer = uplotDataBuffers[i + 1];
			if (buffer) {
				for (let j = 0; j < series.length; j++) {
					const value = series[j];
					buffer[j] = value === null ? NaN : value;
				}
			}
		});

		const finalDataViews = uplotDataBuffers.map((buffer) => buffer.subarray(0, finalDataLength));
		return { views: finalDataViews, latestTimestamp: latestAbsTimestamp };
	});

	// --- Legend Interpolation Logic ---
	const interpolatedValues = $derived.by((): (number | null)[] => {
		const relTime = legendState.relativeTime;
		const data = rawPlotData;

		if (relTime === null || isFFT || !legendState.isActive || !data || data.timestamps.length === 0) return [];

		const latestAbsTimestamp = data.timestamps[data.timestamps.length - 1];
		const targetTime = latestAbsTimestamp + relTime;
		const idx = findTimestampIndex(data.timestamps, targetTime);

		if (idx === 0 || idx >= data.timestamps.length) return plot.series.map(() => null);

		const t1 = data.timestamps[idx - 1];
		const t2 = data.timestamps[idx];

		return data.series_data.map((series) => {
			const y1 = series[idx - 1];
			const y2 = series[idx];
			return lerp(t1, y1, t2, y2, targetTime);
		});
	});

	// --- uPlot Instantiation and Lifecycle ---
	$effect(() => {
		if (!chartContainer) return;

		const legendPlugin: uPlot.Plugin = {
			hooks: {
				setCursor: (u) => {
					const { left, top, idx } = u.cursor;
					if (left === undefined || left < 0 || idx === undefined || idx === null || top === undefined) {
						legendState.isActive = false;
						return;
					}
					legendState.isActive = true;
					legendState.cursorLeft = left;
					legendState.cursorTop = top;
					legendState.chartBounds = chartContainer.getBoundingClientRect();
					if (isFFT) {
						legendState.frequency = u.data[0][idx];
						legendState.values = u.data.slice(1).map((d) => d[idx] ?? null);
						legendState.relativeTime = null;
					} else {
						legendState.relativeTime = u.posToVal(left, 'x');
						legendState.frequency = null;
					}
				},
				setSeries: (u, seriesIdx) => {
					if (seriesIdx === null) {
						legendState.isActive = false;
						legendState.relativeTime = null;
					}
				}
			}
		};

		const finalOptions: uPlot.Options = { ...options, plugins: [legendPlugin] };
        const uplotInstance = new uPlot(finalOptions, [[]], chartContainer);
        uplot = uplotInstance;

        const resizeObserver = new ResizeObserver((entries) => {
            if (!entries.length) return;
            const { width, height } = entries[0].contentRect;
            uplotInstance.setSize({ width, height });
        });
        resizeObserver.observe(chartContainer);

        return () => {
            resizeObserver.disconnect();
            uplotInstance.destroy();
        };
    });

	// Sync plot configuration with the backend
	$effect(() => {
		const _fingerprint = {
			series: plot.series,
			viewType: plot.viewType,
			decimationMethod: plot.decimationMethod,
			windowSeconds: plot.windowSeconds,
			resolutionMultiplier: plot.resolutionMultiplier,
			fftSeconds: plot.fftSeconds,
			fftDetrendMethod: plot.fftDetrendMethod
		};
		chartState.syncPlotWithBackend(plot);
	});

	// Reactive effect for updating uPlot data
	$effect(() => {
		const uplotInstance = uplot;
		if (!uplotInstance) return;

		const data = preparedData;
		const isPaused = isEffectivelyPaused;
		const currentWindow = plot.windowSeconds;
        const isCurrentlyFFT = isFFT;

		if (data) {
			plot.hasData = true;

			if (!isPaused) {
				latestTimestamp = data.latestTimestamp;
				if (!isCurrentlyFFT) {
					uplotInstance.setScale('x', { min: -currentWindow, max: 0 });
				}
			}
			
			uplotInstance.setData(data.views, isCurrentlyFFT);

		} else if (plot.hasData) {
			uplotInstance.setData([[]], false);
			plot.hasData = false;
		}
	});

</script>

<div class="relative h-full w-full">
	<div bind:this={chartContainer} class="min-h-0 h-full w-full"></div>
</div>

{#if legendState.isActive && legendState.chartBounds}
	<CustomLegend
		isActive={legendState.isActive}
		viewType={plot.viewType}
		relativeTime={legendState.relativeTime}
		frequency={legendState.frequency}
		series={plot.series}
		values={isFFT ? legendState.values : interpolatedValues}
		cursorLeft={legendState.cursorLeft}
		cursorTop={legendState.cursorTop}
		chartBounds={legendState.chartBounds}
	/>
{/if}