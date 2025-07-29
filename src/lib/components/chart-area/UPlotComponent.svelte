<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { invoke } from '@tauri-apps/api/core';
	import { chartState } from '$lib/states/chartState.svelte';
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import type { PlotData } from '$lib/bindings/PlotData';
	import type { PortState } from '$lib/bindings/PortState';
	import CustomLegend from '$lib/components/chart-area/legend/CustomLegend.svelte';

	// --- Props ---
	let {
		plot,
		latestTimestamp = $bindable(),
		connectionState = 'Streaming'
	}: {
		plot: PlotConfig;
		latestTimestamp?: number;
		connectionState?: PortState | null;
	} = $props();

	// --- Derived Values ---
	const options = $derived(plot.uPlotOptions);
	const seriesDataKeys = $derived(plot.series.map((s) => s.dataKey));
	const isFFT = $derived(plot.viewType === 'fft');
	const isEffectivelyPaused = $derived(chartState.isPaused || plot.isPaused);
	const isStreaming = $derived(connectionState === 'Streaming');

	// --- Component State ---
	let chartContainer: HTMLDivElement;
	let uplot: uPlot | undefined;
	let plotDimensions = $state({ width: 0, height: 0 });

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

	// --- GC-FRIENDLY BUFFERS ---
	let uplotDataBuffers = $state.raw<Float64Array[]>([]);

	// --- Buffer Allocation Effect ---
	$effect(() => {
		const numSeries = seriesDataKeys.length;
		const maxRate = plot.maxSamplingRate;
		const winSecs = plot.windowSeconds;

		if (numSeries === 0 || maxRate <= 0 || winSecs <= 0) {
			if (uplotDataBuffers.length > 0) {
				uplotDataBuffers = [];
			}
			return;
		}

		const requiredPoints = Math.ceil(maxRate * winSecs * 2);

		const needsReallocation =
			uplotDataBuffers.length !== numSeries + 1 ||
			(uplotDataBuffers.length > 0 && uplotDataBuffers[0].length < requiredPoints);

		if (needsReallocation) {
			console.log(
				`Reallocating plot buffers: ${numSeries} series, ${requiredPoints} points (max rate: ${maxRate.toFixed(2)} Hz)`
			);
			const newBuffers = [new Float64Array(requiredPoints)];
			for (let i = 0; i < numSeries; i++) {
				newBuffers.push(new Float64Array(requiredPoints));
			}
			uplotDataBuffers = newBuffers;
		}
	});

	// --- Loop Controller Effect ---
	$effect(() => {
		const isReady = seriesDataKeys.length > 0 && !!uplot && uplotDataBuffers.length > 0;
		if (!isReady) {
			uplot?.setData([[]]);
			return;
		}
		let animationFrameId: number;
		let isLoopRunning = true;
    	let isFetching = false;
		async function fetchData() {
			try {
				const command = isFFT ? 'get_latest_fft_data' : 'get_latest_plot_data';
				const requiredPoints = Math.round(plotDimensions.width * (plot.resolutionMultiplier / 100.0));
				const args = isFFT
					? {
							keys: seriesDataKeys,
							windowSeconds: plot.fftSeconds,
							detrend: plot.fftDetrendMethod
						}
					: {
							keys: seriesDataKeys,
							windowSeconds: plot.windowSeconds,
							numPoints: requiredPoints,
							decimation: plot.decimationMethod
						};
				const result = await invoke<PlotData>(command, args);
				plot.hasData = result.timestamps.length > 0;
				if (result.timestamps.length > 0) {
					if (uplotDataBuffers[0].length < result.timestamps.length) {
						console.warn('Buffer too small, skipping frame.');
						return;
					}
					const absoluteTimestamps = isFFT ? result.timestamps.slice(1) : result.timestamps;
					const finalDataLength = absoluteTimestamps.length;
					const latestAbsTimestamp = absoluteTimestamps[finalDataLength - 1];
					latestTimestamp = latestAbsTimestamp;
					if (!isFFT) {
						uplot!.setScale('x', {
							min: -plot.windowSeconds,
							max: 0
						});
						const relativeXValues = uplotDataBuffers[0];
						for (let i = 0; i < finalDataLength; i++) {
							relativeXValues[i] = absoluteTimestamps[i] - latestAbsTimestamp;
						}
					} else {
						uplotDataBuffers[0].set(absoluteTimestamps);
					}
					result.series_data.forEach((series, i) => {
						if (uplotDataBuffers[i + 1]) {
							const finalSeries = isFFT ? series.slice(1) : series;
							uplotDataBuffers[i + 1].set(finalSeries);
						}
					});
					const finalDataViews = uplotDataBuffers.map((buffer) =>
						buffer.subarray(0, finalDataLength)
					);
					uplot!.setData(finalDataViews, isFFT);
				}
			} catch (e) {
				console.error(`Failed to fetch plot data for ${plot.viewType} view:`, e);
				plot.hasData = false;
			}
		}
		async function mainLoop() {
			if (isLoopRunning) {
				if (!isEffectivelyPaused && !isFetching && isStreaming) {
					isFetching = true;
					await fetchData();
					isFetching = false;
				}
				animationFrameId = requestAnimationFrame(mainLoop);
			}
		}
		mainLoop();
		return () => {
			isLoopRunning = false;
			cancelAnimationFrame(animationFrameId);
		};
	});

	// --- Effect to fetch interpolated data for the legend via IPC ---
	$effect(() => {
		const time =
			legendState.relativeTime === null ? null : (latestTimestamp ?? 0) + legendState.relativeTime;
		const keys = seriesDataKeys;

		if (time === null || keys.length === 0 || isFFT || !legendState.isActive) {
			legendState.values = [];
			return;
		}

		async function fetchLegendValues() {
			try {
				const interpolatedValues = await invoke<(number | null)[]>(
					'get_interpolated_values_at_time',
					{ keys, time }
				);
				legendState.values = interpolatedValues;
			} catch (e) {
				console.error('Failed to fetch interpolated values for legend:', e);
				legendState.values = keys.map(() => null);
			}
		}

		fetchLegendValues();
	});

	// --- uPlot Instance Lifecycle Effect ---
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
   						legendState.values = u.data.slice(1).map(seriesData => seriesData[idx] ?? null);
						legendState.relativeTime = null;
					} else {
						legendState.relativeTime = u.posToVal(left, 'x');
						legendState.frequency = null;
					}
				},
				setSeries: (u: uPlot, seriesIdx: number | null) => {
					if (seriesIdx === null) {
						legendState.isActive = false;
						legendState.relativeTime = null;
					}
				}
			}
		};

		const finalOptions: uPlot.Options = {
			...options,
			plugins: [...(options.plugins || []), legendPlugin]
		};

		const newUplotInstance = new uPlot(finalOptions, [[]], chartContainer);
		uplot = newUplotInstance;

		const resizeObserver = new ResizeObserver((entries) => {
			if (!entries.length) return;
			const { width, height } = entries[0].contentRect;
			plotDimensions = { width, height };
			newUplotInstance.setSize({ width, height });
		});
		resizeObserver.observe(chartContainer);

		return () => {
			resizeObserver.disconnect();
			newUplotInstance.destroy();
			if (uplot === newUplotInstance) {
				uplot = undefined;
			}
		};
	});

	// --- View Type Change Effect ---
	$effect(() => {
		const _ = plot.viewType;
		if (uplot) {
			uplot.setData([[]]);
		}
		if (uplotDataBuffers.length > 0) {
			uplotDataBuffers.forEach((buffer) => buffer.fill(0));
		}
	});
</script>

<div class="relative h-full w-full">
	<div bind:this={chartContainer} class="h-full w-full min-h-0"></div>
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
