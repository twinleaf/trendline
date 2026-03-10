<script lang="ts">
    import { chartState, type PlotConfig } from "$lib/states/chartState.svelte";
    import type { RowSelectionState } from "@tanstack/table-core";
    import {
        columns,
        type TreeRow,
    } from "$lib/components/chart-area/data-table/column";
    import DataTable from "$lib/components/chart-area/data-table/DataTable.svelte";
    import PlotSettings from "$lib/components/chart-area/PlotSettings.svelte";
    import { Button } from "$lib/components/ui/button/";
    import * as Popover from "$lib/components/ui/popover";
    import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
    import * as Tabs from "$lib/components/ui/tabs";
    import { Settings } from "@lucide/svelte";

    type Props = {
        plot: PlotConfig;
        treeData: TreeRow[];
    };

    let { plot = $bindable(), treeData }: Props = $props();
    let isPlotSettingsOpen = $state(false);
    let draftRowSelection = $state<RowSelectionState>(plot.rowSelection);
    let isCommitting = false;

    $effect(() => {
        const confirmed = plot.rowSelection;
        if (!isCommitting) {
            draftRowSelection = confirmed;
        }
    });

    function handleRowSelectionCommit(rowSelection: RowSelectionState) {
        draftRowSelection = rowSelection;
        isCommitting = true;
        chartState.setPlotSelection(plot, rowSelection).finally(() => {
            isCommitting = false;
            draftRowSelection = plot.rowSelection;
        });
    }
</script>

<Popover.Root bind:open={isPlotSettingsOpen}>
    <Popover.Trigger>
        <Button
            variant="ghost"
            size="icon"
            class={isPlotSettingsOpen
                ? "text-primary hover:text-primary"
                : "text-muted-foreground hover:text-foreground"}
            aria-label="Plot settings"
        >
            <Settings class="size-5" />
        </Button>
    </Popover.Trigger>
    <Popover.Content
        class="w-[600px] overflow-hidden p-0 pt-1"
        onCloseAutoFocus={(e) => e.preventDefault()}
    >
        <Tabs.Root bind:value={plot.activeTab} class="flex w-full flex-col p-2">
            <Tabs.List class="grid w-full grid-cols-2">
                <Tabs.Trigger value="selection">Plot Selection</Tabs.Trigger>
                <Tabs.Trigger value="settings">Plot Settings</Tabs.Trigger>
            </Tabs.List>

            <Tabs.Content value="selection" class="mt-2 min-h-0">
                <div class="h-[40vh] min-h-0">
                    <ScrollArea class="h-full">
                        <DataTable
                            {columns}
                            data={treeData}
                            getSubRows={(row: TreeRow) => row.subRows}
                            bind:expanded={plot.expansion}
                            rowSelection={draftRowSelection}
                            onRowSelectionCommit={handleRowSelectionCommit}
                        />
                    </ScrollArea>
                </div>
            </Tabs.Content>
            <Tabs.Content value="settings" class="mt-2 min-h-0">
                <div class="h-[40vh] min-h-0">
                    <ScrollArea class="h-full">
                        <div class="p-2">
                            <PlotSettings bind:plot />
                        </div>
                    </ScrollArea>
                </div>
            </Tabs.Content>
        </Tabs.Root>
    </Popover.Content>
</Popover.Root>
