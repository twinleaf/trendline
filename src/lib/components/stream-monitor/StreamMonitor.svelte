<script lang="ts">
    import { deviceState } from "$lib/states/deviceState.svelte";
    import { streamMonitorState } from "$lib/states/streamMonitorState.svelte";
    import {
        isVirtualUrl,
        isVirtualKey,
        sortUiDevicesByRoute,
    } from "$lib/utils";
    import type { UiStream } from "$lib/bindings/UiStream";
    import type { UiColumnMeta } from "$lib/bindings/UiColumnMeta";
    import type { TreeRow } from "$lib/components/chart-area/data-table/column";
    import type { UiColumnKey } from "$lib/bindings/UiColumnKey";
    import { onDestroy } from "svelte";

    import { columns } from "$lib/components/stream-monitor/data-table/column";
    import DataTable from "$lib/components/stream-monitor/data-table/DataTable.svelte";
    import StreamCell from "./data-table/StreamCell.svelte";
    import { Button } from "$lib/components/ui/button";
    import { ScrollArea } from "$lib/components/ui/scroll-area";
    import { Settings, Eraser } from "@lucide/svelte";
    import * as Popover from "$lib/components/ui/popover";
    import * as AlertDialog from "$lib/components/ui/alert-dialog";
    import type {
        ExpandedState,
        RowSelectionState,
    } from "@tanstack/table-core";
    import type { UiDevice } from "$lib/bindings/UiDevice";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { Label } from "$lib/components/ui/label";

    let isInitialSelectionDone = false;
    let isWipeDialogOpen = $state(false);
    let isSettingsOpen = $state(false);
    let showDesc = $state(true);
    let includeVirtual = $state(false);

    onDestroy(() => {
        void streamMonitorState.destroy();
    });

    // --- Derived Data ---
    interface MonitorItem {
        id: string;
        name: string;
        depth: number;
        units?: string;
        dataKey: UiColumnKey;
    }

    interface GroupedMonitorItems {
        headerName: string;
        headerRoute: string | null;
        items: MonitorItem[];
    }

    let treeData = $derived.by((): TreeRow[] => {
        const topLevelNodes: TreeRow[] = [];

        const selected = deviceState.selectedDevices as UiDevice[];
        const virtuals: UiDevice[] = (() => {
            if (!includeVirtual) return [];
            const out: UiDevice[] = [];
            for (const entry of deviceState.devices) {
                for (const d of entry.devices) {
                    if (isVirtualUrl(d.url)) out.push(d);
                }
            }
            return out;
        })();
        const devicesForTree: UiDevice[] = [...selected];
        for (const v of virtuals) {
            if (
                !devicesForTree.some(
                    (d) => d.url === v.url && d.route === v.route,
                )
            ) {
                devicesForTree.push(v);
            }
        }

        const sortedDevices = devicesForTree.sort(sortUiDevicesByRoute);

        for (const device of sortedDevices) {
            const streamNodes: TreeRow[] = [];
            const sortedStreams = [...device.streams].sort(
                (a: UiStream, b: UiStream) =>
                    a.meta.name.localeCompare(b.meta.name),
            );

            for (const stream of sortedStreams) {
                const sortedColumns = [...stream.columns].sort(
                    (a: UiColumnMeta, b: UiColumnMeta) =>
                        a.name.localeCompare(b.name),
                );

                const columnNodes: TreeRow[] = [];
                for (const column of sortedColumns) {
                    const dataKey = {
                        route: device.route,
                        stream_id: stream.meta.stream_id,
                        column_id: column.index,
                    };
                    columnNodes.push({
                        id: JSON.stringify(dataKey),
                        type: "column",
                        name: column.name,
                        units: column.units ?? "",
                        dataKey,
                    });
                }

                const streamId = `${device.url}:${device.route}:${stream.meta.stream_id}`;
                streamNodes.push({
                    id: streamId,
                    type: "stream",
                    name: stream.meta.name,
                    subRows: columnNodes,
                });
            }

            const deviceId = `${device.url}:${device.route}`;
            topLevelNodes.push({
                id: deviceId,
                type: "device",
                name: device.meta.name,
                subRows: streamNodes,
            });
        }

        return topLevelNodes;
    });

    let selectedItems = $derived.by((): GroupedMonitorItems[] => {
        const selectedKeys = Object.keys(streamMonitorState.rowSelection);
        const leafNodeKeys = selectedKeys.filter((key) => key.startsWith("{"));

        const selectedDevicesMap = new Map<string, UiDevice>();
        for (const key of leafNodeKeys) {
            try {
                const dataKey = JSON.parse(key) as UiColumnKey;
                const device = deviceState.getDevice(dataKey.route);
                if (device) {
                    selectedDevicesMap.set(device.url + device.route, device);
                }
            } catch {}
        }

        const nameCounts = new Map<string, number>();
        for (const device of selectedDevicesMap.values()) {
            nameCounts.set(
                device.meta.name,
                (nameCounts.get(device.meta.name) || 0) + 1,
            );
        }

        const grouped = new Map<string, GroupedMonitorItems>();

        for (const key of leafNodeKeys) {
            try {
                const dataKey = JSON.parse(key);
                if (!includeVirtual && isVirtualKey(dataKey)) continue;
                const device = deviceState.getDevice(dataKey.route);
                const stream = device?.streams.find(
                    (s) => s.meta.stream_id === dataKey.stream_id,
                );
                const column = stream?.columns.find(
                    (c) => c.index === dataKey.column_id,
                );

                if (device && stream && column) {
                    const deviceKey = device.url + device.route;

                    if (!grouped.has(deviceKey)) {
                        const nameIsDuplicated =
                            (nameCounts.get(device.meta.name) || 0) > 1;
                        grouped.set(deviceKey, {
                            headerName: device.meta.name,
                            headerRoute:
                                nameIsDuplicated && device.route
                                    ? device.route
                                    : null,
                            items: [],
                        });
                    }

                    const isMultiColumn = stream.columns.length > 1;
                    grouped.get(deviceKey)!.items.push({
                        id: key,
                        name: showDesc ? column.description : column.name,
                        depth: isMultiColumn ? 1 : 0,
                        units: column.units,
                        dataKey: dataKey,
                    });
                }
            } catch (e) {
                console.error("Failed to parse stream monitor key:", key, e);
            }
        }

        return Array.from(grouped.values()).sort((a, b) =>
            a.headerName.localeCompare(b.headerName),
        );
    });

    // --- Helper Functions and Effects ---
    function getAllSelectableIds(nodes: TreeRow[]): {
        selection: string[];
        expansion: string[];
    } {
        const selection: string[] = [];
        const expansion: string[] = [];
        function recurse(items: TreeRow[]) {
            for (const item of items) {
                const subRows = item.subRows;
                if (subRows && subRows.length > 0) {
                    selection.push(item.id);
                    expansion.push(item.id);
                    recurse(subRows);
                } else if (item.type === "column") {
                    selection.push(item.id);
                }
            }
        }
        recurse(nodes);
        return { selection, expansion };
    }

    async function handleWipeAllStatistics() {
        await streamMonitorState.resetAllStatistics();
        isWipeDialogOpen = false;
    }

    $effect(() => {
        if (isInitialSelectionDone) return;

        const { selection, expansion } = getAllSelectableIds(treeData);
        const hasSelectableColumns = selection.some((id) => id.startsWith("{"));
        if (!hasSelectableColumns) return;

        const initialSelection: RowSelectionState = {};
        for (const id of selection) initialSelection[id] = true;
        void streamMonitorState.setRowSelection(initialSelection);

        const initialExpansion: ExpandedState = {};
        for (const id of expansion) initialExpansion[id] = true;
        streamMonitorState.setExpansion(initialExpansion);

        isInitialSelectionDone = true;
    });
</script>

<div
    class="flex h-full w-full flex-col rounded-lg border bg-card p-4 text-card-foreground"
>
    <div class="mb-2 flex items-center justify-end border-b pb-2 gap-1">
        <AlertDialog.Root bind:open={isWipeDialogOpen}>
            <AlertDialog.Trigger aria-label="Wipe all persistent statistics">
                {#snippet child({ props })}
                    <Button
                        {...props}
                        variant="ghost"
                        size="icon"
                        class="text-muted-foreground hover:text-foreground"
                    >
                        <Eraser class="size-5" />
                    </Button>
                {/snippet}
            </AlertDialog.Trigger>
            <AlertDialog.Content
                onCloseAutoFocus={(event) => {
                    event.preventDefault();
                }}
            >
                <AlertDialog.Header>
                    <AlertDialog.Title
                        >Are you absolutely sure?</AlertDialog.Title
                    >
                    <AlertDialog.Description>
                        This action will permanently wipe the statistics AND
                        data for all streams. This cannot be undone.
                    </AlertDialog.Description>
                </AlertDialog.Header>
                <AlertDialog.Footer>
                    <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
                    <AlertDialog.Action onclick={handleWipeAllStatistics}
                        >Wipe All</AlertDialog.Action
                    >
                </AlertDialog.Footer>
            </AlertDialog.Content>
        </AlertDialog.Root>

        <Popover.Root bind:open={isSettingsOpen}>
            <Popover.Trigger aria-label="Configure streams">
                {#snippet child({ props })}
                    <Button
                        {...props}
                        variant="ghost"
                        size="icon"
                        class={isSettingsOpen
                            ? "text-primary hover:text-primary"
                            : "text-muted-foreground hover:text-foreground"}
                    >
                        <Settings class="size-5" />
                    </Button>
                {/snippet}
            </Popover.Trigger>
            <Popover.Content
                class="w-[500px] p-0 pt-1"
                side="bottom"
                align="end"
            >
                <div class="p-2">
                    <div
                        class="mb-2 flex items-center justify-between border-b pb-1"
                    >
                        <h3 class="mb-2 px-1 font-semibold">
                            Channel Selection
                        </h3>
                        <div class="mb-2 px-1 flex items-center gap-1">
                            <Checkbox id="terms" bind:checked={showDesc} />
                            <Label for="terms">Descriptive names</Label>
                        </div>
                        <div class="mb-2 px-1 flex items-center gap-1">
                            <Checkbox id="virt" bind:checked={includeVirtual} />
                            <Label for="virt">Virtual channels</Label>
                        </div>
                    </div>
                    <ScrollArea class="h-full">
                        <div class="max-h-[40vh]">
                            <DataTable
                                {columns}
                                data={treeData}
                                getSubRows={(row) => row.subRows}
                                expanded={streamMonitorState.expansion}
                                rowSelection={streamMonitorState.rowSelection}
                                onExpandedCommit={(expanded) =>
                                    streamMonitorState.setExpansion(expanded)}
                                onRowSelectionCommit={(selection) =>
                                    void streamMonitorState.setRowSelection(
                                        selection,
                                    )}
                            />
                        </div>
                    </ScrollArea>
                </div>
            </Popover.Content>
        </Popover.Root>
    </div>
    <ScrollArea class="min-h-0 flex-1">
        <div class="space-y-4 p-2">
            {#if selectedItems.length > 0}
                {#each selectedItems as group (group.headerName + group.headerRoute)}
                    <div>
                        <div class="mb-2 border-b pb-1">
                            <div class="text-left">
                                <p class="font-semibold">{group.headerName}</p>
                                {#if group.headerRoute}
                                    <p class="text-xs text-muted-foreground">
                                        Route: {group.headerRoute}
                                    </p>
                                {/if}
                            </div>
                        </div>

                        <div class="space-y-2">
                            {#each group.items as item (item.id)}
                                <div>
                                    <StreamCell
                                        name={item.name}
                                        units={item.units}
                                        depth={item.depth}
                                        dataKey={item.dataKey}
                                    />
                                </div>
                            {/each}
                        </div>
                    </div>
                {/each}
            {:else}
                <div
                    class="flex h-full items-center justify-center pt-12 text-center text-muted-foreground"
                >
                    <p>
                        No streams selected. Use the <Settings
                            class="inline size-4 -mt-1"
                        /> icon to configure.
                    </p>
                </div>
            {/if}
        </div>
    </ScrollArea>
</div>
