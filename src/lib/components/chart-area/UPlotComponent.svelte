<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { invoke } from '@tauri-apps/api/core';
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import type { PlotData } from '$lib/bindings/PlotData';

	// --- Props ---
	let { 
        plot,
        latestTimestamp = $bindable() 
    }: {
        plot: PlotConfig,
        latestTimestamp?: number
    } = $props();

	// --- Derived Values ---
	const options = $derived(plot.uPlotOptions);
	const seriesDataKeys = $derived(plot.series.map((s) => s.dataKey));
	const isFFT = $derived(plot.viewType === 'fft');

	// --- Component State ---
	let chartContainer: HTMLDivElement;
	let uplot: uPlot | undefined;
	let startTimeSeconds: number | null = null;
	let plotDimensions = $state({ width: 0, height: 0 });

	// --- GC-FRIENDLY BUFFERS ---
	let uplotDataBuffers = $state<Float64Array[]>([]); 

	// --- Buffer Allocation Effect ---
	$effect(() => {

		const requiredPoints = Math.ceil(plot.maxSamplingRate * plot.windowSeconds) + 10;

		if (seriesDataKeys.length === 0 || requiredPoints <= 10) {
			uplotDataBuffers = [];
			return;
		}

		const numSeries = seriesDataKeys.length;
		const needsReallocation = uplotDataBuffers.length !== numSeries + 1;

		if (needsReallocation) {
			console.log(`Allocating buffers for worst-case size: ${requiredPoints} points`);
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

		async function fetchData() {
			try {
				const command = isFFT ? 'get_latest_fft_data' : 'get_latest_plot_data';
				const requiredPoints = Math.round(plotDimensions.width * 1.5); 
				
				const args = isFFT
					? { keys: seriesDataKeys, windowSeconds: plot.fftSeconds }
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
						console.warn("Buffer too small, skipping frame.");
						return;
					}

					const finalXValues = isFFT ? result.timestamps.slice(1) : result.timestamps;
					const finalDataLength = finalXValues.length;

					latestTimestamp = finalXValues[finalXValues.length - 1];

					if (!isFFT) {
						uplot!.setScale('x', {
							min: latestTimestamp - plot.windowSeconds,
							max: latestTimestamp
						});
					}

					uplotDataBuffers[0].set(finalXValues);
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

		function mainLoop() {
			fetchData();
			animationFrameId = requestAnimationFrame(mainLoop);
		}
		
		mainLoop();

		return () => {
			cancelAnimationFrame(animationFrameId);
		};
	});

	// --- uPlot Instance Lifecycle Effect ---
	$effect(() => {
		if (!chartContainer) return;
		const newUplotInstance = new uPlot(options, [[]], chartContainer);
		uplot = newUplotInstance;

		const resizeObserver = new ResizeObserver((entries) => {
			if (!entries.length) return;
			const { width, height: totalAvailableHeight } = entries[0].contentRect;
			const titleEl = newUplotInstance.root.querySelector('.u-title') as HTMLElement;
			const legendEl = newUplotInstance.root.querySelector('.u-legend') as HTMLElement;
			const titleHeight = titleEl?.offsetHeight ?? 0;
			const legendHeight = legendEl?.offsetHeight ?? 0;
			const plotAreaHeight = totalAvailableHeight - titleHeight - legendHeight;

			plotDimensions = { width, height: plotAreaHeight };

			newUplotInstance.setSize({
				width: width,
				height: Math.max(0, plotAreaHeight)
			});
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
			uplotDataBuffers.forEach(buffer => buffer.fill(0));
		}
		startTimeSeconds = null;
	});
</script>

<div bind:this={chartContainer} class="h-full w-full min-h-0"></div>