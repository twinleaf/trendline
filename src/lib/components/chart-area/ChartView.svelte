<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import { deviceState } from '$lib/states/deviceState.svelte';
	import { type TreeRow } from '$lib/components/chart-area/data-table/column';
	import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';
	import PlotHeader from '$lib/components/chart-area/PlotHeader.svelte';
	import * as Resizable from '$lib/components/ui/resizable';
	import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
	import SplitButton from '$lib/components/chart-area/SplitButton.svelte';
	import type { UiStream } from '$lib/bindings/UiStream';
	import type { ColumnMeta } from '$lib/bindings/ColumnMeta';
	import { sortUiDevicesByRoute } from '$lib/utils';
	import { onDestroy, onMount } from 'svelte';
	import * as ContextMenu from '$lib/components/ui/context-menu/index.js';
	import { ArrowUp, ArrowDown, Download, Plus, Trash2 } from '@lucide/svelte';

	let plots = $derived(chartState.plots);
	let layout = $derived(chartState.layout);

	const MIN_PANE_HEIGHT_PX = 250;
	let totalLayoutHeight = $derived(Object.values(layout).reduce((sum, size) => sum + size, 0));

	// svelte-ignore non_reactive_update
	let chartAreaContainer: HTMLDivElement | null = null;

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

	$effect(() => {
		if (!chartAreaContainer) return;
		const resizeObserver = new ResizeObserver((entries) => {
			const entry = entries[0];
			if (entry) {
				chartState.containerHeight = entry.contentRect.height;
			}
		});
		resizeObserver.observe(chartAreaContainer);
		return () => {
			resizeObserver.disconnect();
		};
	});

	onDestroy(() => {
		chartState.destroy();
	});

</script>

<div class="relative flex h-full w-full flex-col p-4 gap-4">
	<ScrollArea class="flex-1 min-h-0" orientation="vertical" bind:ref={chartAreaContainer}>
		<ContextMenu.Root>
			<ContextMenu.Trigger class="block h-full min-h-full">
				{#if plots.length > 0}
					<div class="flex-1 min-h-full" style={`height: ${totalLayoutHeight}px;`}>
						<Resizable.PaneGroup
							direction="vertical"
							class="w-full h-full gap-4"
							onLayoutChange={(percentages) => {
								if (chartState.layoutMode === 'manual') {
									chartState.updateLayoutFromManualResize(percentages);
								}
							}}
							oncontextmenu={(e) => {
								e.stopPropagation();
							}}
						>
							{#each plots as plot, i (plot.id)}
								{@const plotPixelHeight = layout[plot.id] || 0}
								{@const totalPixelHeightForPercent = totalLayoutHeight > 0 ? totalLayoutHeight : 1}
								{@const defaultSizePercentage = (plotPixelHeight / totalPixelHeightForPercent) * 100}
								<Resizable.Pane
									id={plot.id}
									minSize={(MIN_PANE_HEIGHT_PX / totalPixelHeightForPercent) * 100}
									defaultSize={defaultSizePercentage}
									order={i}
								>
									<ContextMenu.Root>
										<ContextMenu.Trigger class="block h-full w-full">
											<div
												class="flex h-full flex-col gap-2 rounded-lg border p-4 relative"
											>
												<PlotHeader
													bind:plot={plots[i]}
													{treeData}
													onRemove={() => chartState.removePlot(plot.id)}
												/>
												<div class="flex-1 min-h-0">
													<UPlotComponent {plot} bind:latestTimestamp={plot.latestTimestamp} />
												</div>
											</div>
										</ContextMenu.Trigger>
										<ContextMenu.Content class="w-56">
											{@const plotId = plot.id}
											{@const plotIndex = chartState.getPlotIndex(plotId)}
											{@const plotCount = plots.length}
											<ContextMenu.Item onclick={() => chartState.addPlotAbove(plotId)}>
												<Plus class="mr-2 h-4 w-4" />
												Add Plot Above
											</ContextMenu.Item>
											<ContextMenu.Item onclick={() => chartState.addPlotBelow(plotId)}>
												<Plus class="mr-2 h-4 w-4" />
												Add Plot Below
											</ContextMenu.Item>
											<ContextMenu.Separator />
											<ContextMenu.Item
												onclick={() => chartState.movePlot(plotId, 'up')}
												disabled={plotIndex === 0}
											>
												<ArrowUp class="mr-2 h-4 w-4" />
												Move Plot Up
											</ContextMenu.Item>
											<ContextMenu.Item
												onclick={() => chartState.movePlot(plotId, 'down')}
												disabled={plotIndex === plotCount - 1}
											>
												<ArrowDown class="mr-2 h-4 w-4" />
												Move Plot Down
											</ContextMenu.Item>
											<ContextMenu.Separator />
											<ContextMenu.Item onclick={() => chartState.exportPlotToCsv(plotId)}>
												<Download class="mr-2 h-4 w-4" />
												Export to CSV...
											</ContextMenu.Item>
											<ContextMenu.Separator />
											<ContextMenu.Item
												onclick={() => chartState.removePlot(plotId)}
												class="text-destructive focus:text-destructive"
											>
												<Trash2 class="mr-2 h-4 w-4" />
												Delete Plot
											</ContextMenu.Item>
										</ContextMenu.Content>
									</ContextMenu.Root>
								</Resizable.Pane>
								{#if i < plots.length - 1}
									<Resizable.Handle
										withHandle
										onDraggingChange={(isDragging) => {
											if (isDragging) {
												chartState.switchToManualMode();
											}
										}}
									/>
								{/if}
							{/each}
						</Resizable.PaneGroup>
					</div>
				{:else}
					<div class="flex h-full w-full flex-col items-center justify-center text-muted-foreground">
						<h3 class="text-2xl font-semibold">No Plots to Display</h3>
						<p class="mb-4 mt-2 text-sm">Right-click or use the button below to add a new plot.</p>
					</div>
				{/if}
			</ContextMenu.Trigger>
			<ContextMenu.Content>
				<ContextMenu.Item onclick={() => chartState.addPlot()}>
					<Plus class="mr-2 h-4 w-4" />
					Add Plot
				</ContextMenu.Item>
			</ContextMenu.Content>
		</ContextMenu.Root>
	</ScrollArea>

	<div class="absolute z-10 right-6 bottom-6">
		<SplitButton />
	</div>
</div>