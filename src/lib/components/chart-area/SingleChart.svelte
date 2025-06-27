<script lang="ts">
	import uPlot from 'uplot';
	import { onMount, onDestroy } from 'svelte';

	let {
		options,
		data,
		onViewChange = (min: number, max: number) => {}
	} = $props<{
		options: uPlot.Options;
		data: uPlot.AlignedData;
		onViewChange?: (min: number, max: number) => void;
	}>();

	let chartContainer: HTMLDivElement;
	let uplot: uPlot | null = null;
	let isInitialized = false;

	onMount(() => {
		if (chartContainer) {
			const extendedOptions: uPlot.Options = {
				...options,
				hooks: {
					setScale: [
						(self, key) => {
							if (key === 'x' && isInitialized) {
								const min = self.scales.x.min ?? 0;
								const max = self.scales.x.max ?? 1;
								onViewChange(min, max);
							}
						}
					]
				}
			};

			uplot = new uPlot(extendedOptions, data, chartContainer);
			isInitialized = true;
		}
	});
	

	$effect(() => {
		if (uplot) {
			uplot.setData(data, false);
		}
	});

	onDestroy(() => {
		if (uplot) {
			uplot.destroy();
			uplot = null;
		}
		isInitialized = false;
	});
</script>

<div class="relative min-h-[400px] w-full flex-grow" bind:this={chartContainer}>
	{#if !options.series || options.series.length <= 1}
		<div class="absolute inset-0 flex items-center justify-center text-muted-foreground">
			<p>No valid data columns found for this route.</p>
		</div>
	{/if}
</div>