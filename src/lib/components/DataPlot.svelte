<script lang="ts">
	import uPlot from 'uplot';
	import { onMount, onDestroy } from 'svelte';

	let {
		options,
		data
	} = $props<{
		options: uPlot.Options;
		data: uPlot.AlignedData;
	}>();

	let chartContainer: HTMLDivElement;
	let uplot: uPlot | null = null;

	onMount(() => {
		if (chartContainer) {
			const initialOptions = { ...options, series: options.series || [] };
			uplot = new uPlot(initialOptions, data, chartContainer);
		}
	});
	

	$effect(() => {
		if (uplot) {
			uplot.setData(data, true);
		}
	});

	onDestroy(() => {
		if (uplot) {
			uplot.destroy();
			uplot = null;
		}
	});
</script>

<div class="relative min-h-[400px] w-full flex-grow" bind:this={chartContainer}>
	{#if !options.series || options.series.length <= 1}
		<div class="absolute inset-0 flex items-center justify-center text-muted-foreground">
			<p>No valid data columns found for this route.</p>
		</div>
	{/if}
</div>