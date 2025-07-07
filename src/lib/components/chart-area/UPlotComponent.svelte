<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import type { PlotData } from '$lib/bindings/PlotData';

	// --- Props ---
	let { plot }: { plot: PlotConfig } = $props();

	// --- Derived Values ---
	const options = $derived(plot.uPlotOptions);
	const seriesDataKeys = $derived(plot.series.map((s) => s.dataKey));

	// --- Component State ---
	let chartContainer: HTMLDivElement;
	let uplot: uPlot | undefined;
	let startTimeSeconds: number | null = null;

	// --- Data Fetching Loop (runs continuously) ---
	onMount(() => {
		let animationFrameId: number;

		async function fetchData() {
			// Guard against running before the plot is initialized by the $effect
			if (seriesDataKeys.length === 0 || !uplot) {
				uplot?.setData([[]]);
				return;
			}
            const numPoints = Math.round(uplot.width * 2);
			try {
				const result = await invoke<PlotData>('get_latest_plot_data', {
					keys: seriesDataKeys,
					windowSeconds: 30.0,
                    numPoints: numPoints,
				});

				if (result.timestamps.length > 0) {
					if (startTimeSeconds === null) {
						startTimeSeconds = Date.now() / 1000 - result.timestamps[0];
					}
					const absoluteTimestamps = result.timestamps.map((t) => startTimeSeconds! + t);
					const finalData: uPlot.AlignedData = [
                        new Float64Array(absoluteTimestamps),
                        ...result.series_data.map(s => 
                            new Float64Array(s.map(value => value === null ? NaN : value))
                        )
                    ];
					uplot.setData(finalData, true);
				}
			} catch (e) {
				console.error('Failed to fetch plot data:', e);
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

	// --- uPlot Instance Lifecycle (destroy & recreate on changes) ---
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
</script>

<div bind:this={chartContainer} class="h-full w-full min-h-0"></div>