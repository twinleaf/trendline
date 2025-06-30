<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import type { DataColumnId } from '$lib/bindings/DataColumnId';
	import type { PlotData } from '$lib/bindings/PlotData';

	let {
		options,
		seriesDataKeys,
		windowSizeMs = 120000 // 2-minute sliding window
	} = $props<{
		options: uPlot.Options;
		seriesDataKeys: DataColumnId[];
		windowSizeMs?: number;
	}>();

	let chartContainer: HTMLDivElement;
	let uplot: uPlot | undefined;

	let animationFrameId: number;
	let dataIntervalId: number;

	let plotData: uPlot.AlignedData = [[]];

	onMount(() => {
		// Set up the plot immediately. It will be empty at first.
		uplot = new uPlot(options, plotData, chartContainer);

		async function fetchData() {
			if (seriesDataKeys.length === 0) {
				plotData = [[]]; // Clear data if no keys
				return;
			}
			try {
				const result = await invoke<PlotData>('get_buffered_plot_data', {
					keys: seriesDataKeys
				});
				// Combine timestamps and series into the final uPlot format
				const finalData: uPlot.AlignedData = [result.timestamps, ...result.series_data];
				plotData = finalData;
			} catch (e) {
				console.error('Failed to fetch plot data:', e);
			}
		}

		function update() {
			if (!uplot) return;

			const now = Date.now();
			// uPlot expects timestamps in seconds, not milliseconds
			const scale = {
				min: (now - windowSizeMs) / 1000,
				max: now / 1000
			};
			
			// On every animation frame:
			// 1. Give uPlot the latest data we have (from the last fetchData interval)
			// 2. Tell it what time window to display
			uplot.setData(plotData, false);
			uplot.setScale('x', scale);

			animationFrameId = requestAnimationFrame(update);
		}

		// --- Start the loops ---
		fetchData(); // Initial data fetch
		dataIntervalId = setInterval(fetchData, 200); // Poll for new data 5x per second
		animationFrameId = requestAnimationFrame(update); // Start the smooth render loop

		// --- Cleanup ---
		return () => {
			cancelAnimationFrame(animationFrameId);
			clearInterval(dataIntervalId);
			uplot?.destroy();
		};
	});

	// Effect to handle resizing the chart container
	$effect(() => {
		if (uplot && chartContainer) {
			uplot.setSize({
				width: chartContainer.clientWidth,
				height: options.height ?? 300 // Provide a sensible default height
			});
		}
	});
</script>

<div bind:this={chartContainer} class="w-full min-h-[300px]">
    <!-- Display a message if there's nothing to plot -->
    {#if seriesDataKeys.length === 0}
        <div class="absolute inset-0 flex items-center justify-center text-muted-foreground">
			<p>No valid data columns to plot for this unit.</p>
		</div>
    {/if}
</div>