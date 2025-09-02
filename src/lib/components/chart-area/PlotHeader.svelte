<script lang="ts">
	import type { PlotConfig } from '$lib/states/chartState.svelte';
	import type { TreeRow } from '$lib/components/chart-area/data-table/column';
	import PlotControls from '$lib/components/chart-area/PlotControls.svelte';
	import { Button } from '$lib/components/ui/button/';
	import { Toggle } from '$lib/components/ui/toggle/';
	import { Play, Pause, Trash2, ChartLine, ChartColumn } from '@lucide/svelte';
	import { chartState } from '$lib/states/chartState.svelte';

	type Props = {
		plot: PlotConfig;
		treeData: TreeRow[];
		onRemove: (id: string) => void;
	};

	let { plot = $bindable(), treeData, onRemove }: Props = $props();

	const isEffectivelyPaused = $derived(plot.isPaused);
</script>

<div class="flex justify-between items-center">
	<input
		onclick={(e) => e.stopPropagation()}
		onkeydown={(e) => e.stopPropagation()}
		type="text"
		class="text-lg font-semibold bg-transparent focus:bg-background rounded-md px-2 -mx-2 outline-none focus:ring-1 focus:ring-ring"
		bind:value={plot.title}
	/>
	<div class="flex items-center gap-2">
		<Button
			variant="ghost"
			size="icon"
			aria-label={isEffectivelyPaused ? 'Play chart' : 'Pause chart'}
			onclick={(e) => {
				e.stopPropagation();
				chartState.handleLocalPlotToggle(plot);
			}}
		>
			{#if isEffectivelyPaused}
				<Play class="size-5" />
			{:else}
				<Pause class="size-5" />
			{/if}
		</Button>
		<Toggle
			aria-label="Toggle plot view type"
			pressed={plot.viewType === 'fft'}
			onclick={(e) => {
				e.stopPropagation();
				plot.viewType = plot.viewType === 'timeseries' ? 'fft' : 'timeseries';
			}}
		>
			{#if plot.viewType === 'timeseries'}
				<ChartLine class="size-5" />
			{:else}
				<ChartColumn class="size-5" />
			{/if}
		</Toggle>
		<PlotControls bind:plot {treeData} />
		<Button
			variant="ghost"
			size="icon"
			onclick={(e) => {
				e.stopPropagation();
				onRemove(plot.id);
			}}
			aria-label="Remove Plot"
		>
			<Trash2 class="size-5 text-destructive/80 hover:text-destructive" />
		</Button>
	</div>
</div>