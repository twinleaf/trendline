<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';
	import * as Resizable from '$lib/components/ui/resizable';

	let renderPlan = $derived(chartState.renderPlan);

    function calculateDeviceHeight(plotCount: number): number {
		const BASE_VH = 15;   // Base height for the header, padding, etc.
		const PER_PLOT_VH = 18; // Allocate 18vh for each plot.
		const MAX_VH = 90;      // Cap the total height at 90% of the viewport.

		const calculatedHeight = BASE_VH + plotCount * PER_PLOT_VH;
		return Math.min(calculatedHeight, MAX_VH);
	}
</script>
{#if renderPlan.length > 0}
	<div class="w-full">
		{#each renderPlan as devicePlots (devicePlots.device.url + devicePlots.device.route)}
			<div
				class="mb-4 flex flex-col"
				style:height={`${calculateDeviceHeight(devicePlots.plots.length)}vh`}
			>
				<section class="flex min-h-0 flex-1 flex-col gap-2 rounded-lg border p-4">
					<h2 class="mb-2 text-lg font-semibold tracking-tight">
						{devicePlots.device.meta.name}
						<span class="text-sm font-mono text-muted-foreground"
							>({devicePlots.device.route})</span
						>
					</h2>
					<Resizable.PaneGroup direction="vertical" class="flex-1">
						{#each devicePlots.plots as plotConfig (plotConfig.title)}
							<Resizable.Pane defaultSize={100 / devicePlots.plots.length} minSize={3}>
								<div class="flex h-full flex-col p-2">
									<UPlotComponent
                                        options={plotConfig.uPlotOptions}
                                        seriesDataKeys={plotConfig.series.map((s) => s.dataKey)}
                                        bind:hasData={plotConfig.hasData}
                                        plotTitle={plotConfig.title}
                                    />
								</div>
							</Resizable.Pane>

							{#if devicePlots.plots[devicePlots.plots.length - 1] !== plotConfig}
								<Resizable.Handle withHandle />
							{/if}
						{/each}
					</Resizable.PaneGroup>
				</section>
			</div>
		{/each}
	</div>
{:else}
	<div class="flex h-full items-center justify-center text-muted-foreground">
		<p>Select one or more devices to begin plotting.</p>
	</div>
{/if}