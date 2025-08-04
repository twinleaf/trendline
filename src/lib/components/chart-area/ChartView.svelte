<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import { deviceState } from '$lib/states/deviceState.svelte';
	import { type TreeRow } from '$lib/components/chart-area/data-table/column';
	import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';
	import PlotHeader from '$lib/components/chart-area/PlotHeader.svelte';
	import * as Resizable from '$lib/components/ui/resizable';
	import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
	import SplitButton from '$lib/components/chart-area/SplitButton.svelte';
	import type { UiStream } from '$lib/bindings/UiStream';
	import type { ColumnMeta } from '$lib/bindings/ColumnMeta';
	import { sortUiDevicesByRoute } from '$lib/utils';
	import { onDestroy } from 'svelte';
	import * as ContextMenu from '$lib/components/ui/context-menu/index.js';
	import { ArrowUp, ArrowDown, Download, Plus, Trash2, Scaling, ClipboardCopy } from '@lucide/svelte';

	// Svelte 5 state management
	let plots = $derived(chartState.plots);
	let layout = $derived(chartState.layout);
	let targetedPlotId: string | null = $state(null);
	let isMenuOpen = $state(false);
	let chartAreaContainer: HTMLDivElement | null = $state(null);

	const MIN_PANE_HEIGHT_PX = 250;
	let totalLayoutHeight = $derived(Object.values(layout).reduce((sum, size) => sum + size, 0));

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
		<ContextMenu.Root open={isMenuOpen} onOpenChange={(v) => (isMenuOpen = v)}>
			<ContextMenu.Trigger
				class="block h-full min-h-full"
				oncontextmenu={(e) => {
					const target = e.target as HTMLElement;
					const plotPane = target.closest('.plot-pane');

					if (plotPane?.id) {
						targetedPlotId = plotPane.id;
					} else {
						targetedPlotId = null;
					}
				}}
			>
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
						>
							{#each plots as plot, i (plot.id)}
								<Resizable.Pane
									id={plot.id}
									minSize={(MIN_PANE_HEIGHT_PX / totalLayoutHeight) * 100}
									defaultSize={(layout[plot.id] / totalLayoutHeight) * 100}
									order={i}
									class="overflow-hidden plot-pane"
								>
									<div class="flex h-full flex-col gap-2 rounded-lg border p-4 relative">
										<PlotHeader
											bind:plot={plots[i]}
											{treeData}
											onRemove={() => chartState.removePlot(plot.id)}
										/>
										<div class="flex-1 min-h-0">
											<UPlotComponent {plot} bind:latestTimestamp={plot.latestTimestamp} />
										</div>
									</div>
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
					<div
						class="flex h-full w-full flex-col items-center justify-center text-muted-foreground"
					>
						<h3 class="text-2xl font-semibold">No Plots to Display</h3>
						<p class="mb-4 mt-2 text-sm">Right-click or use the button below to add a new plot.</p>
					</div>
				{/if}
			</ContextMenu.Trigger>
			<ContextMenu.Content class="w-56">
				{#if targetedPlotId}
					{@const plotId = targetedPlotId}
					{@const plotIndex = chartState.getPlotIndex(plotId)}
					{@const plotCount = plots.length}
					{@const currentHeightKey = chartState.getPlotHeightPercentageKey(plotId)}
					<ContextMenu.Item
						onclick={() => {
							isMenuOpen = false;
							chartState.addPlotAbove(plotId);
						}}
					>
						<Plus class="mr-2 h-4 w-4" />
						Add Plot Above
					</ContextMenu.Item>
					<ContextMenu.Item
						onclick={() => {
							isMenuOpen = false;
							chartState.addPlotBelow(plotId);
						}}
					>
						<Plus class="mr-2 h-4 w-4" />
						Add Plot Below
					</ContextMenu.Item>
					<ContextMenu.Separator />
					<ContextMenu.Item
						onclick={() => {
							isMenuOpen = false;
							chartState.movePlot(plotId, 'up');
						}}
						disabled={plotIndex === 0}
					>
						<ArrowUp class="mr-2 h-4 w-4" />
						Move Plot Up
					</ContextMenu.Item>
					<ContextMenu.Item
						onclick={() => {
							isMenuOpen = false;
							chartState.movePlot(plotId, 'down');
						}}
						disabled={plotIndex === plotCount - 1}
					>
						<ArrowDown class="mr-2 h-4 w-4" />
						Move Plot Down
					</ContextMenu.Item>
					<ContextMenu.Separator />

					<ContextMenu.Sub>
						<ContextMenu.SubTrigger>
							<Scaling class="mr-2 h-4 w-4" />
							Set Plot Height
						</ContextMenu.SubTrigger>
						<ContextMenu.SubContent class="w-48">
							<ContextMenu.RadioGroup value={currentHeightKey}>
								<ContextMenu.RadioItem
									value="25"
									disabled={plots.length === 1}
									onclick={() => {
										isMenuOpen = false;
										chartState.setPlotHeight(plotId, 25);
									}}
								>
									25% of Viewport
								</ContextMenu.RadioItem>
								<ContextMenu.RadioItem
									value="33"
									disabled={plots.length === 1}
									onclick={() => {
										isMenuOpen = false;
										chartState.setPlotHeight(plotId, 33.33);
									}}
								>
									33% of Viewport
								</ContextMenu.RadioItem>
								<ContextMenu.RadioItem
									value="50"
									disabled={plots.length === 1}
									onclick={() => {
										isMenuOpen = false;
										chartState.setPlotHeight(plotId, 50);
									}}
								>
									50% of Viewport
								</ContextMenu.RadioItem>
								<ContextMenu.RadioItem
									value="100"
									disabled={plots.length === 1}
									onclick={() => {
										isMenuOpen = false;
										chartState.setPlotHeight(plotId, 100);
									}}
								>
									100% of Viewport
								</ContextMenu.RadioItem>
							</ContextMenu.RadioGroup>
						</ContextMenu.SubContent>
					</ContextMenu.Sub>
					 <ContextMenu.Separator />

                    <ContextMenu.Item
                        onclick={() => {
                            isMenuOpen = false;
                            // Calls the "Copy View" function
                            chartState.copyPlotViewToClipboard(plotId);
                        }}
                    >
                        <ClipboardCopy class="mr-2 h-4 w-4" />
                        <span>Copy CSV...</span>
                    </ContextMenu.Item>

                    <ContextMenu.Item
                        onclick={() => {
                            isMenuOpen = false;
                            // Calls the "Save Raw" function
                            chartState.savePlotRawData(plotId);
                        }}
                    >
                        <Download class="mr-2 h-4 w-4" />
                        <span>Save as CSV...</span>
                    </ContextMenu.Item>
					<ContextMenu.Separator />
					<ContextMenu.Item
						onclick={() => {
							isMenuOpen = false;
							chartState.removePlot(plotId);
						}}
						class="text-destructive focus:text-destructive"
					>
						<Trash2 class="mr-2 h-4 w-4" />
						Delete Plot
					</ContextMenu.Item>
				{:else}
					<ContextMenu.Item
						onclick={() => {
							isMenuOpen = false;
							chartState.addPlot();
						}}
					>
						<Plus class="mr-2 h-4 w-4" />
						Add Plot
					</ContextMenu.Item>
				{/if}
			</ContextMenu.Content>
		</ContextMenu.Root>
	</ScrollArea>

	<div class="absolute z-10 right-6 bottom-6">
		<SplitButton />
	</div>
</div>