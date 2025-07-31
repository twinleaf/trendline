<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { invoke } from '@tauri-apps/api/core';
	import { chartState } from '$lib/states/chartState.svelte';
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import type { PortState } from '$lib/bindings/PortState';
	import type { PipelineId } from '$lib/bindings/PipelineId';
	import CustomLegend from '$lib/components/chart-area/legend/CustomLegend.svelte';
	import { untrack } from 'svelte';

	// --- Props (Unchanged) ---
	let {
		plot,
		latestTimestamp = $bindable(),
		connectionState = 'Streaming'
	}: {
		plot: PlotConfig;
		latestTimestamp?: number;
		connectionState?: PortState | null;
	} = $props();

	// --- DERIVED STATE (from global chartState) ---
	const options = $derived(plot.uPlotOptions);
	const isFFT = $derived(plot.viewType === 'fft');
	const isEffectivelyPaused = $derived(chartState.isPaused || plot.isPaused);
	const plotData = $derived(chartState.plotsData.get(plot.id));

	// --- Component State ---
	let chartContainer: HTMLDivElement;
	let uplot = $state.raw<uPlot | undefined>(undefined);

	// --- State for the custom legend ---
	let legendState = $state({
		isActive: false,
		relativeTime: null as number | null,
		frequency: null as number | null,
		values: [] as (number | null)[],
		cursorLeft: null as number | null,
		cursorTop: null as number | null,
		chartBounds: null as DOMRect | null
	});

	// --- GC-FRIENDLY BUFFERS & Allocation (Unchanged) ---
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
			const newBuffers = [new Float64Array(requiredPoints)];
			for (let i = 0; i < numSeries; i++) newBuffers.push(new Float64Array(requiredPoints));
			uplotDataBuffers = newBuffers;
		}
	});

	// --- Data-Driven Render Trigger (Reacts to global state) ---
	$effect(() => {
		const dataToRender = plotData;
		if (!uplot || isEffectivelyPaused) return;

		// Handle cases where data is not available (e.g., initial render, error)
		if (!dataToRender || dataToRender.timestamps.length === 0) {
			if (plot.hasData) uplot.setData([[]], false); // Clear the plot if it had data
			plot.hasData = false;
			return;
		}

		plot.hasData = true;

		if (uplotDataBuffers.length === 0 || uplotDataBuffers[0].length < dataToRender.timestamps.length) {
			console.warn('Buffer not ready or too small for plot, skipping frame.');
			return;
		}

		const absoluteTimestamps = dataToRender.timestamps;
		const finalDataLength = absoluteTimestamps.length;
		const latestAbsTimestamp = absoluteTimestamps[finalDataLength - 1];
		latestTimestamp = latestAbsTimestamp; // Bind the latest timestamp out

		// The rest of the rendering logic is the same, just using the new data source
		if (!isFFT) {
			uplot.setScale('x', { min: -plot.windowSeconds, max: 0 });
			const relativeXValues = uplotDataBuffers[0];
			for (let i = 0; i < finalDataLength; i++) {
				relativeXValues[i] = absoluteTimestamps[i] - latestAbsTimestamp;
			}
		} else {
			uplotDataBuffers[0].set(absoluteTimestamps);
		}

		dataToRender.series_data.forEach((series, i) => {
			if (uplotDataBuffers[i + 1]) {
				uplotDataBuffers[i + 1].set(series);
			}
		});

		const finalDataViews = uplotDataBuffers.map((buffer) => buffer.subarray(0, finalDataLength));
		uplot.setData(finalDataViews, isFFT);
	});

	// --- Effect to fetch interpolated data for the legend (Updated) ---
	$effect(() => {
		const time =
			legendState.relativeTime === null ? null : (latestTimestamp ?? 0) + legendState.relativeTime;
		if (time === null || isFFT || !legendState.isActive) {
			legendState.values = [];
			return;
		}

		// KEY CHANGE #2: Use pipeline IDs from the PlotConfig's map
		const pipelineIds = untrack(() =>
			plot.series
				.map((s) => plot.pipelineMap.get(JSON.stringify(s.dataKey))?.timeseriesId)
				.filter((id): id is PipelineId => !!id)
		);

		if (pipelineIds.length === 0) return;

		// Use the correct command for getting interpolated values from pipelines
		invoke<(number | null)[]>('get_interpolated_values', { pipelineIds, time })
			.then((interpolatedValues) => {
				legendState.values = interpolatedValues;
			})
			.catch((e) => {
				console.error('Failed to fetch interpolated values for legend:', e);
				legendState.values = pipelineIds.map(() => null);
			});
	});

	// --- uPlot Instance Lifecycle Effect (Unchanged) ---
	$effect(() => {
		if (!chartContainer) return;

		const legendPlugin: uPlot.Plugin = {
			hooks: {
				setCursor: (u: uPlot) => {
					const { left, top, idx } = u.cursor;
					if (left === undefined || left < 0 || idx === undefined || idx === null) {
						legendState.isActive = false;
						return;
					}
					legendState.isActive = true;
					legendState.cursorLeft = left ?? null;
					legendState.cursorTop = top ?? null;
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
				setSeries: (_, seriesIdx: number | null) => {
					if (seriesIdx === null) {
						legendState.isActive = false;
						legendState.relativeTime = null;
					}
				}
			}
		};

		const finalOptions: uPlot.Options = { ...options, plugins: [...(options.plugins || []), legendPlugin] };
		const newUplotInstance = new uPlot(finalOptions, [[]], chartContainer);
		uplot = newUplotInstance;

		const resizeObserver = new ResizeObserver((entries) => {
			if (!entries.length) return;
			const { width, height } = entries[0].contentRect;
			newUplotInstance.setSize({ width, height });
		});
		resizeObserver.observe(chartContainer);

		return () => {
			resizeObserver.disconnect();
			newUplotInstance.destroy();
			if (uplot === newUplotInstance) uplot = undefined;
		};
	});

	// --- View Type Change Effect (Unchanged) ---
	$effect(() => {
		const _ = plot.viewType;
		if (uplot) {
			uplot.setData([[]]);
			plot.hasData = false;
		}
		if (uplotDataBuffers.length > 0) uplotDataBuffers.forEach((buffer) => buffer.fill(0));
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
		values={legendState.values}
		cursorLeft={legendState.cursorLeft}
		cursorTop={legendState.cursorTop}
		chartBounds={legendState.chartBounds}
	/>
{/if}