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

	let plotData: uPlot.AlignedData = [[]];

	onMount(() => {
		// Set up the plot immediately. It will be empty at first.
		uplot = new uPlot(options, plotData, chartContainer);

		async function fetchData() {
			if (seriesDataKeys.length === 0 || !uplot) return;
			const scale = uplot.scales.x;
            if (!scale || !scale.min || !scale.max) return;
			try {
				const result = await invoke<PlotData>('get_plot_data', {
                    keys: seriesDataKeys,
                    minTime: scale.min,
                    maxTime: scale.max,
				});
                
				if (result.timestamps.length > 0) {
				    const finalData: uPlot.AlignedData = [result.timestamps, ...result.series_data];
				    uplot.setData(finalData);
                }
			} catch (e) {
				console.error('Failed to fetch plot data:', e);
			}
		}

		function update() {
			if (!uplot) return;
			const now = Date.now() / 1000;
			uplot.setScale('x', {
                min: now - windowSizeMs / 1000,
                max: now
            });
		}

		function mainLoop() {
            update();
            fetchData();
            animationFrameId = requestAnimationFrame(mainLoop);
        }

		// --- Start the loops ---
		animationFrameId = requestAnimationFrame(mainLoop);

		// --- Cleanup ---
		return () => {
			cancelAnimationFrame(animationFrameId);
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