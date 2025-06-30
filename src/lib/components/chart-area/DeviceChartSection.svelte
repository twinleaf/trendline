<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';

	let renderPlan = $derived(chartState.renderPlan);
</script>

<div class="flex h-full w-full flex-col gap-6 overflow-y-auto p-4">
	{#if renderPlan.length > 0}
		{#each renderPlan as devicePlots (devicePlots.device.url + devicePlots.device.route)}
			<section class="flex w-full flex-col gap-2 rounded-lg border p-4">
				<h2 class="mb-2 text-lg font-semibold tracking-tight">
					Device: {devicePlots.device.meta.name} 
                    <span class="text-sm font-mono text-muted-foreground">({devicePlots.device.route})</span>
				</h2>

				{#each devicePlots.plots as plotConfig (plotConfig.title)}
					<div class="plot-container">
						<h3 class="mb-1 text-sm font-medium text-muted-foreground">
							{plotConfig.title}
						</h3>

						<UPlotComponent
							options={plotConfig.uPlotOptions}
							seriesDataKeys={plotConfig.series.map(s => s.dataKey)}
						/>
					</div>
				{/each}
			</section>
		{/each}
	{:else}
		<div class="flex h-full items-center justify-center text-muted-foreground">
			<p>Select one or more devices to begin plotting.</p>
		</div>
	{/if}
</div>