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
	import { onDestroy, onMount, untrack } from 'svelte';

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
        {#if plots.length > 0}
            <div class="flex-1 min-h-0">
                <div style={`height: ${totalLayoutHeight}px; min-height: 100%;`}>
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
							{@const plotPixelHeight = layout[plot.id] || 0}
							{@const totalPixelHeightForPercent = totalLayoutHeight > 0 ? totalLayoutHeight : 1}
							{@const defaultSizePercentage = (plotPixelHeight / totalPixelHeightForPercent) * 100}
                            <Resizable.Pane
								id={plot.id}
                                minSize={(MIN_PANE_HEIGHT_PX / totalPixelHeightForPercent) * 100}
								defaultSize={defaultSizePercentage}
								order={i}
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
            </div>
        {:else}
            <div class="flex h-full w-full flex-col items-center justify-center text-muted-foreground">
                <h3 class="text-2xl font-semibold">No Plots to Display</h3>
                <p class="mb-4 mt-2 text-sm">Get started by adding a new plot.</p>
            </div>
        {/if}
	</ScrollArea>

	<div class="absolute z-10 right-6 bottom-6">
		<SplitButton />
	</div>
</div>