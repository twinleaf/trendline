<!-- svelte-ignore non_reactive_update -->
<!-- src/lib/components/chart-area/UPlotComponent.svelte -->
<script lang="ts">
	import uPlot from 'uplot';
	import 'uplot/dist/uPlot.min.css';
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import type { DataColumnId } from '$lib/bindings/DataColumnId';
	import type { PlotData } from '$lib/bindings/PlotData';

	let {
		options,
		seriesDataKeys,
		windowSizeMs = 60000,
		plotTitle = 'this stream',
		hasData = $bindable(false),
	} = $props<{
		options: uPlot.Options;
		seriesDataKeys: DataColumnId[];
		windowSizeMs?: number;
		plotTitle?: string;
        hasData?: boolean;
	}>();

	// --- State for this component ---
	let chartContainer: HTMLDivElement;
	let uplot: uPlot | undefined;
	let width = $state(0);
	let height = $state(0);
	let startTimeSeconds: number | null = null;
    let timedOut = $state(false);


	onMount(() => {
		const timeoutId = setTimeout(() => {
            if (!hasData) {
                timedOut = true;
                uplot?.setData([[]]); // Clear any empty axes from the initial render
            }
        }, 3000); 
		uplot = new uPlot({ ...options }, [[]], chartContainer);
		let animationFrameId: number;

		async function fetchData() {
			if (seriesDataKeys.length === 0 || !uplot) return;

			try {
				const result = await invoke<PlotData>('get_latest_plot_data', {
					keys: seriesDataKeys,
					windowSeconds: windowSizeMs / 1000.0,
				});

				if (result.timestamps.length > 0) {
					hasData = true;
					if (startTimeSeconds === null) {
                        const nowSeconds = Date.now() / 1000;
                        startTimeSeconds = nowSeconds - result.timestamps[0];
                    }
					const absoluteTimestamps = result.timestamps.map(t => startTimeSeconds! + t);
					const finalData: uPlot.AlignedData = [absoluteTimestamps, ...result.series_data];
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

		animationFrameId = requestAnimationFrame(mainLoop);

		return () => {
			cancelAnimationFrame(animationFrameId);
			clearTimeout(timeoutId);
			uplot?.destroy();
		};
	});

	// The effect only needs to handle resizing. No time logic.
	$effect(() => {
		if (uplot && width > 0 && height > 0) {
			const titleEl = chartContainer.querySelector('.u-title') as HTMLElement;
			const legendEl = chartContainer.querySelector('.u-legend') as HTMLElement;

			const titleHeight = titleEl?.offsetHeight ?? 0;
			const legendHeight = legendEl?.offsetHeight ?? 0;


			const plotAreaHeight = height - titleHeight - legendHeight;

			uplot.setSize({ width, height: Math.max(0, plotAreaHeight) });
		}
	});
</script>

<div
    class="relative shrink w-full min-h-0 h-full"
    bind:clientWidth={width}
    bind:clientHeight={height}
>
    <div
        bind:this={chartContainer}
        class="h-full w-full transition-opacity"
        class:opacity-0={!hasData}
    ></div>

    {#if !hasData}
        <div class="absolute inset-0 flex items-center justify-center text-muted-foreground">
            {#if timedOut}
                <p>No data available for {plotTitle}.</p>
            {:else}
                <p class="animate-pulse">Awaiting data...</p>
            {/if}
        </div>
    {/if}
</div>