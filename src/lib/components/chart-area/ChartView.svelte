<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import { deviceState } from '$lib/states/deviceState.svelte';
	import PlotControls from '$lib/components/chart-area/PlotControls.svelte';
	import { type TreeRow } from '$lib/components/chart-area/data-table/column';
	import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';
	import * as Resizable from '$lib/components/ui/resizable';
	import { Button } from '$lib/components/ui/button/';
	import { Toggle } from '$lib/components/ui/toggle/';
	import { Play, Pause, Trash2, Plus, ChartLine, ChartColumn } from '@lucide/svelte';
	import type { UiStream } from '$lib/bindings/UiStream';
	import type { ColumnMeta } from '$lib/bindings/ColumnMeta';
	import { sortUiDevicesByRoute } from '$lib/utils';

	let plots = $derived(chartState.plots);
	let layout = $derived(chartState.layout);

	const MIN_PANE_HEIGHT_PX = 200;
	let totalResizableHeight = $derived(Object.values(layout).reduce((sum, size) => sum + size, 0));

	// --- Event Handlers ---

	function handleGlobalKeydown(event: KeyboardEvent) {
		if (event.key === ' ') {
			if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement)
				return;
			event.preventDefault();
			chartState.togglePause();
		}
	}

	// --- Data Transformation ---

	let treeData = $derived.by((): TreeRow[] => {
		const topLevelNodes: TreeRow[] = [];

		for (const portData of deviceState.devices) {
			const sortedDevices = [...portData.devices].sort(sortUiDevicesByRoute);

			for (const device of sortedDevices) {
				const sortedStreams = [...device.streams].sort((a: UiStream, b: UiStream) =>
					a.meta.name.localeCompare(b.meta.name)
				);

				const streamNodes: TreeRow[] = [];
				for (const stream of sortedStreams) {
					const sortedColumns = [...stream.columns].sort((a: ColumnMeta, b: ColumnMeta) =>
						a.name.localeCompare(b.name)
					);

					const columnNodes: TreeRow[] = [];
					const streamSamplingRate = stream.effective_sampling_rate ?? 0;
					for (const column of sortedColumns) {
						const dataKey = {
							port_url: device.url,
							device_route: device.route,
							stream_id: stream.meta.stream_id,
							column_index: column.index
						};
						columnNodes.push({
							id: JSON.stringify(dataKey),
							type: 'column',
							name: column.name,
							units: column.units ?? '',
							description: column.description ?? '',
							dataKey: dataKey,
							samplingRate: streamSamplingRate
						});
					}
					streamNodes.push({
						id: `${device.url}:${device.route}:${stream.meta.stream_id}`,
						type: 'stream',
						name: stream.meta.name,
						subRows: columnNodes,
						samplingRate: streamSamplingRate
					});
				}

				topLevelNodes.push({
					id: `${device.url}:${device.route}`,
					type: 'device',
					name: device.meta.name,
					device: device,
					subRows: streamNodes
				});
			}
		}
		return topLevelNodes;
	});
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="relative flex h-full w-full flex-col">
	<div class="h-full w-full overflow-y-auto">
		<div class="p-4 flex flex-col gap-4 h-full">
			{#if plots.length > 0}
				<div class="flex justify-end">
					<Button onclick={() => chartState.addPlot()}>
						<Plus class="mr-2 h-4 w-4" /> Add Plot
					</Button>
				</div>
				<div class="flex-1 min-h-0">
					<div style={`height: ${totalResizableHeight}px;`}>
						<Resizable.PaneGroup
							direction="vertical"
							class="w-full h-full gap-4"
							onLayoutChange={(sizes: number[]) => {
								const newLayout: Record<string, number> = {};
								sizes.forEach((percent, i) => {
									const plotId = plots[i]?.id;
									if (plotId) {
										newLayout[plotId] = (percent / 100) * totalResizableHeight;
									}
								});
								chartState.layout = newLayout;
							}}
						>
							{#each plots as plot, i (plot.id)}
								{@const plotHeight = layout[plot.id] || 0}
								<Resizable.Pane
									minSize={
										totalResizableHeight > 0
											? (MIN_PANE_HEIGHT_PX / totalResizableHeight) * 100
											: 0
									}
									defaultSize={totalResizableHeight > 0 ? (plotHeight / totalResizableHeight) * 100 : 0}
									order={i}
								>
									<div class="flex h-full flex-col gap-2 rounded-lg border p-4 relative">
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
													aria-label={plot.isPaused ? 'Play chart' : 'Pause chart'}
													onclick={(e) => {
														e.stopPropagation();
														plot.isPaused = !plot.isPaused;
													}}
												>
													{#if plot.isPaused}
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
												<PlotControls bind:plot={plots[i]} {treeData} />
												<Button
													variant="ghost"
													size="icon"
													onclick={(e) => {
														e.stopPropagation();
														chartState.removePlot(plot.id);
													}}
													aria-label="Remove Plot"
												>
													<Trash2 class="size-5 text-destructive/80 hover:text-destructive" />
												</Button>
											</div>
										</div>
										<div class="flex-1 min-h-0">
											<UPlotComponent {plot} bind:latestTimestamp={plot.latestTimestamp} />
										</div>
									</div>
								</Resizable.Pane>
								{#if i < plots.length - 1}
									<Resizable.Handle withHandle />
								{/if}
							{/each}
						</Resizable.PaneGroup>
					</div>
				</div>
			{:else}
				<div class="flex h-full w-full flex-col items-center justify-center text-muted-foreground">
					<h3 class="text-2xl font-semibold">No Plots to Display</h3>
					<p class="mb-4 mt-2 text-sm">Get started by adding a new plot.</p>
					<Button onclick={() => chartState.addPlot()}>
						<Plus class="mr-2 h-4 w-4" /> Add New Plot
					</Button>
				</div>
			{/if}
		</div>
	</div>
</div>