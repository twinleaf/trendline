<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';
	import * as Resizable from '$lib/components/ui/resizable';

	let renderPlan = $derived(chartState.renderPlan);
</script>

<div class="h-full w-full overflow-y-auto">
	{#if renderPlan.length > 0}
		<Resizable.PaneGroup direction="vertical">
			{#each renderPlan as devicePlots (devicePlots.device.url + devicePlots.device.route)}
				<Resizable.Pane defaultSize={50} minSize={25}>
					<div class="flex h-full flex-col gap-2 overflow-y-auto p-4">
						<section class="flex w-full flex-col gap-2 rounded-lg border p-4">
							<h2 class="mb-2 text-lg font-semibold tracking-tight">
								{devicePlots.device.meta.name}
								<span class="text-sm font-mono text-muted-foreground"
									>({devicePlots.device.route})</span
								>
							</h2>

							{#each devicePlots.plots as plotConfig (plotConfig.title)}
								<div class="plot-container">
									<h3 class="mb-1 text-sm font-medium text-muted-foreground">
										{plotConfig.title}
									</h3>
									<UPlotComponent
										options={plotConfig.uPlotOptions}
										seriesDataKeys={plotConfig.series.map((s) => s.dataKey)}
									/>
								</div>
							{/each}
						</section>
					</div>
				</Resizable.Pane>
				{#if renderPlan[renderPlan.length - 1] !== devicePlots}
					<Resizable.Handle withHandle />
				{/if}
			{/each}
		</Resizable.PaneGroup>
	{:else}
		<div class="flex h-full items-center justify-center text-muted-foreground">
			<p>Select one or more devices to begin plotting.</p>
		</div>
	{/if}
</div>