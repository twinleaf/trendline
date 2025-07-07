<script lang="ts">
    import { chartState } from '$lib/states/chartState.svelte';
    import { deviceState } from '$lib/states/deviceState.svelte';
    import DataTable from '$lib/components/chart-area/data-table/DataTable.svelte';
    import { columns, type TreeRow } from '$lib/components/chart-area/data-table/column';
    import UPlotComponent from '$lib/components/chart-area/UPlotComponent.svelte';
    import * as Resizable from '$lib/components/ui/resizable';
    import * as Popover from "$lib/components/ui/popover/index.js";
    import { Button } from '$lib/components/ui/button/';
    import { Settings, Trash2, Plus} from '@lucide/svelte';
    import type { ExpandedState } from '@tanstack/table-core';


    let plots = $derived(chartState.plots);
    let layout = $derived(chartState.layout);

    const MIN_PANE_HEIGHT_PX = 200;
    let totalResizableHeight = $derived(Object.values(layout).reduce((sum, size) => sum + size, 0));

    let openPopoverId = $state<string | null>(null);

    function handleRemoveClick(plotId: string) {
        chartState.removePlot(plotId);
        if (openPopoverId === plotId) {
            openPopoverId = null;
        }
    }

	let treeData = $derived.by((): TreeRow[] => {
        const topLevelNodes: TreeRow[] = [];

        for (const portData of deviceState.devices) {
            for (const device of portData.devices) {
                const streamNodes: TreeRow[] = [];
                for (const stream of device.streams) {
                    const columnNodes: TreeRow[] = [];
                    for (const column of stream.columns) {
                        const dataKey = {
                            port_url: device.url,
                            device_route: device.route,
                            stream_id: stream.meta.stream_id,
                            column_index: column.index,
                        };
                        columnNodes.push({
                            id: JSON.stringify(dataKey),
                            type: 'column',
                            name: column.name,
                            units: column.units ?? '',
                            description: column.description ?? '',
                            dataKey: dataKey, 
                        });
                    }
                    streamNodes.push({
                        id: `${device.url}:${device.route}:${stream.meta.stream_id}`,
                        type: 'stream',
                        name: stream.meta.name,
                        subRows: columnNodes,
                    });
                }

                topLevelNodes.push({
                    id: `${device.url}:${device.route}`,
                    type: 'device',
                    name: device.meta.name,
                    device: device,
                    subRows: streamNodes,
                });
            }
        }
        return topLevelNodes;
    });


    let initialExpandedState = $derived.by((): ExpandedState => {
        const expanded: Record<string, boolean> = {};
        for (const deviceNode of treeData) {
            if (deviceNode.type === 'device') {
                expanded[deviceNode.id] = true;
            }
        }
        return expanded;
    });
</script>
<div class="relative flex h-full w-full flex-col p-4 gap-4">
    {#if plots.length > 0}
        <div class="flex justify-end">
            <Button onclick={() => chartState.addPlot()}>
                <Plus class="mr-2 h-4 w-4" /> Add Plot
            </Button>
        </div>
        <div class="flex-1 min-h-0 overflow-y-auto">
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
                            minSize={totalResizableHeight > 0 ? (MIN_PANE_HEIGHT_PX / totalResizableHeight) * 100 : 0}
                            defaultSize={totalResizableHeight > 0 ? (plotHeight / totalResizableHeight) * 100 : 0}
                            order={i}
                        >
                            <section class="flex h-full flex-col gap-2 rounded-lg border p-4">
                                <div class="flex justify-between items-center">
                                    <input type="text" bind:value={plot.title} class="text-lg font-semibold ..."/>
                                    <div class="flex items-center gap-2">
                                        <Popover.Root>
                                            <Popover.Trigger aria-label="Plot settings">
                                                <Settings class="size-5 text-muted-foreground" />
                                            </Popover.Trigger>
                                            <Popover.Content class="w-[600px]">
                                                <div class="max-h-[50vh] overflow-y-auto p-2">
                                                    <DataTable 
                                                        {columns} 
                                                        data={treeData} 
                                                        getSubRows={(row: TreeRow) => row.subRows}
                                                        initialExpanded={initialExpandedState}
                                                        bind:rowSelection={plot.rowSelection}
                                                    />
                                                </div>
                                            </Popover.Content>
                                        </Popover.Root>

                                        <Button 
                                                variant="ghost" 
                                                size="icon" 
                                                onclick={() => handleRemoveClick(plot.id)}
                                                aria-label="Remove Plot"
                                            >
                                                <Trash2 class="size-5 text-destructive/80 hover:text-destructive" />
                                        </Button>
                                    </div>
                                </div>
                                <div class="flex-1 min-h-0">
                                    <UPlotComponent {plot} />
                                </div>
                            </section>
                        </Resizable.Pane>
                        {#if i < plots.length - 1}
                            <Resizable.Handle withHandle/>
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
