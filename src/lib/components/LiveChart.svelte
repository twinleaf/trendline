<script lang="ts">
	import { chartStore } from '$lib/stores/chart.store.svelte';
	import DataPlot from '$lib/components/DataPlot.svelte';
	import ChartControls from '$lib/components/ChartControls.svelte';
</script>

<div class="flex h-full w-full flex-col gap-4 overflow-y-auto p-2">
	{#if chartStore.charts.length > 0}
		<div class="flex-shrink-0">
			<ChartControls
				onFetchData={() => chartStore.fetchAllData()}
				isLoading={chartStore.isFetchingAll}
			/>
		</div>

		{#each chartStore.charts as chart (chart.id)}
			<div class="flex w-full flex-col rounded-lg border p-2">
				<DataPlot options={chart.options} data={chart.data} />
			</div>
		{/each}
	{:else}
		<div class="flex h-full items-center justify-center text-muted-foreground">
			<p>Select one or more streams to begin plotting.</p>
		</div>
	{/if}
</div>