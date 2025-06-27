<script lang="ts">
	import { chartStore } from '$lib/stores/chart.store.svelte';
	import DataPlot from '$lib/components/chart-area/SingleChart.svelte';

</script>

<div class="flex h-full w-full flex-col gap-6 overflow-y-auto p-4">
	{#if chartStore.displays.length > 0}
		{#each chartStore.displays as display (display.id)}
			<section class="flex w-full flex-col gap-2 rounded-lg border p-4">
				<h2 class="mb-2 text-lg font-semibold tracking-tight">
					Device: {display.deviceName} <span class="text-sm font-mono text-muted-foreground">({display.id})</span>
				</h2>

				{#each display.plotGroups as group (group.id)}
					<div class="plot-container">
						<h3 class="mb-1 text-sm font-medium text-muted-foreground">
							Unit: {group.unit}
						</h3>
						{#key `${group.id}-${group.options.scales?.x?.min}` }
							<DataPlot
								options={group.options}
								data={group.data}
							/>
						{/key}
					</div>
				{/each}
			</section>
		{/each}
	{:else}
		<div class="flex h-full items-center justify-center text-muted-foreground">
			<p>Select one or more streams to begin plotting.</p>
		</div>
	{/if}
</div>