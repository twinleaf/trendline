<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
	import { streamMonitorState } from '$lib/states/streamMonitorState.svelte';
	import { sortUiDevicesByRoute } from '$lib/utils';
	import type { UiStream } from '$lib/bindings/UiStream';
	import type { ColumnMeta } from '$lib/bindings/ColumnMeta';
	import type { TreeRow } from '$lib/components/chart-area/data-table/column';
	import type { DataColumnId } from '$lib/bindings/DataColumnId';
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';

	import { columns } from '$lib/components/stream-monitor/data-table/column';
	import DataTable from '$lib/components/stream-monitor/data-table/DataTable.svelte';
	import StreamCell from './data-table/StreamCell.svelte';
	import { Button } from '$lib/components/ui/button';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import { Settings, Eraser } from '@lucide/svelte';
	import * as Popover from '$lib/components/ui/popover';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import type { ExpandedState, RowSelectionState } from '@tanstack/table-core';

	let isInitialSelectionDone = false;
	let isWipeDialogOpen = $state(false);

	// --- COMPONENT LIFECYCLE MANAGEMENT ---
	onMount(() => {
		streamMonitorState.initPolling();
		return () => {
			streamMonitorState.destroy();
		};
	});

	// Reactive effect to update pipelines when selection changes
	$effect(() => {
		if (isInitialSelectionDone) {
			streamMonitorState.updatePipelineSelection();
		}
	});

	// --- Derived Data (no changes here) ---
	interface MonitorItem {
		id: string;
		name: string;
		depth: number;
		units?: string;
		dataKey: DataColumnId;
		group: string;
	}

	let treeData = $derived.by((): TreeRow[] => {
		const topLevelNodes: TreeRow[] = [];
		for (const portData of deviceState.devices) {
			const sortedDevices = [...portData.devices].sort(sortUiDevicesByRoute);
			for (const device of sortedDevices) {
				const streamNodes: TreeRow[] = [];
				const sortedStreams = [...device.streams].sort((a: UiStream, b: UiStream) =>
					a.meta.name.localeCompare(b.meta.name)
				);
				for (const stream of sortedStreams) {
					const sortedColumns = [...stream.columns].sort((a: ColumnMeta, b: ColumnMeta) =>
						a.name.localeCompare(b.name)
					);
					const columnNodes: TreeRow[] = [];
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
							dataKey: dataKey
						});
					}
					const streamId = `${device.url}:${device.route}:${stream.meta.stream_id}`;
					streamNodes.push({
						id: streamId,
						type: 'stream',
						name: stream.meta.name,
						subRows: columnNodes
					});
				}
				const deviceId = `${device.url}:${device.route}`;
				topLevelNodes.push({
					id: deviceId,
					type: 'device',
					name: device.meta.name,
					subRows: streamNodes
				});
			}
		}
		return topLevelNodes;
	});

	let selectedItems = $derived.by(() => {
		const items: MonitorItem[] = [];
		const selectedKeys = Object.keys(streamMonitorState.rowSelection);
		const leafNodeKeys = selectedKeys.filter((key) => key.startsWith('{'));

		for (const key of leafNodeKeys) {
			try {
				const dataKey = JSON.parse(key);
				const device = deviceState.getDevice(dataKey.port_url, dataKey.device_route);
				const stream = device?.streams.find((s) => s.meta.stream_id === dataKey.stream_id);
				const column = stream?.columns.find((c) => c.index === dataKey.column_index);

				if (device && stream && column) {
					const isMultiColumn = stream.columns.length > 1;
					items.push({
						id: key,
						name: isMultiColumn ? column.name : stream.meta.name,
						depth: isMultiColumn ? 1 : 0,
						units: column.units,
						dataKey: dataKey,
						group: device.meta.name
					});
				}
			} catch (e) {
				console.error('Failed to parse stream monitor key:', key, e);
			}
		}

		const grouped = items.reduce(
			(acc, item) => {
				if (!acc[item.group]) acc[item.group] = [];
				acc[item.group].push(item);
				return acc;
			},
			{} as Record<string, MonitorItem[]>
		);
		return Object.entries(grouped).sort(([a], [b]) => a.localeCompare(b));
	});

	// --- Helper Functions ---
	function getAllSelectableIds(nodes: TreeRow[]): { selection: string[]; expansion: string[] } {
		const selection: string[] = [];
		const expansion: string[] = [];
		function recurse(items: TreeRow[]) {
			for (const item of items) {
				const subRows = item.subRows;
				if (subRows && subRows.length > 0) {
					selection.push(item.id);
					expansion.push(item.id);
					recurse(subRows);
				} else if (item.type === 'column') {
					selection.push(item.id);
				}
			}
		}
		recurse(nodes);
		return { selection, expansion };
	}

	function getAllDataKeys(nodes: TreeRow[]): DataColumnId[] {
		const keys: DataColumnId[] = [];
		function recurse(items: TreeRow[]) {
			for (const item of items) {
				if (item.type === 'column' && item.dataKey) keys.push(item.dataKey);
				if (item.subRows) recurse(item.subRows);
			}
		}
		recurse(nodes);
		return keys;
	}

	async function handleWipeAllStatistics() {
		const allKeys = getAllDataKeys(treeData);
		if (allKeys.length > 0) {
			await invoke('reset_stream_statistics', { keys: allKeys });
		}
		isWipeDialogOpen = false;
	}

	$effect(() => {
		if (treeData.length > 0 && !isInitialSelectionDone) {
			const { selection, expansion } = getAllSelectableIds(treeData);

			const initialSelection: RowSelectionState = {};
			for (const id of selection) initialSelection[id] = true;
			streamMonitorState.rowSelection = initialSelection;

			const initialExpansion: ExpandedState = {};
			for (const id of expansion) initialExpansion[id] = true;
			streamMonitorState.expansion = initialExpansion;

			isInitialSelectionDone = true;
		}
	});
</script>

<div class="flex h-full w-full flex-col rounded-lg border bg-card p-4 text-card-foreground">
	<div class="mb-2 flex items-center justify-end border-b pb-2">
		<AlertDialog.Root bind:open={isWipeDialogOpen}>
			<AlertDialog.Trigger aria-label="Wipe all persistent statistics">
				{#snippet child({ props })}
					<Button {...props} variant="ghost" size="icon">
						<Eraser class="size-5 text-muted-foreground" />
					</Button>
				{/snippet}
			</AlertDialog.Trigger>
			<AlertDialog.Content>
				<AlertDialog.Header>
					<AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
					<AlertDialog.Description>
						This action will permanently wipe the persistent statistics for all streams. This cannot
						be undone.
					</AlertDialog.Description>
				</AlertDialog.Header>
				<AlertDialog.Footer>
					<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
					<AlertDialog.Action onclick={handleWipeAllStatistics}>Wipe All</AlertDialog.Action>
				</AlertDialog.Footer>
			</AlertDialog.Content>
		</AlertDialog.Root>

		<Popover.Root>
			<Popover.Trigger aria-label="Configure streams">
				{#snippet child({ props })}
					<Button {...props} variant="ghost" size="icon">
						<Settings class="size-5 text-muted-foreground" />
					</Button>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content class="w-[500px] p-0" side="bottom" align="end">
				<div class="p-2">
					<h3 class="mb-2 px-2 font-semibold">Channel Selection</h3>
					<ScrollArea class="h-full">
						<div class="max-h-[40vh]">
							<DataTable
								{columns}
								data={treeData}
								getSubRows={(row) => row.subRows}
								bind:expanded={streamMonitorState.expansion}
								bind:rowSelection={streamMonitorState.rowSelection}
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
				{#each selectedItems as [groupName, items] (groupName)}
					<div>
						<h4 class="mb-2 border-b pb-1 text-base font-semibold">{groupName}</h4>
						<div class="space-y-2">
							{#each items as item (item.id)}
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
						No streams selected. Use the <Settings class="inline size-4 -mt-1" /> icon to
						configure.
					</p>
				</div>
			{/if}
		</div>
	</ScrollArea>
</div>