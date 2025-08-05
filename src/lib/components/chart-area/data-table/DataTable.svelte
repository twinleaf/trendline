<script lang="ts" generics="TData extends TreeRow, TValue">
	import {
		type ColumnDef,
		type Row,
		type RowSelectionState,
		type ExpandedState,
		type ColumnFiltersState,
		getCoreRowModel,
		getExpandedRowModel,
		getFilteredRowModel,
		getSortedRowModel,
		type SortingState,
		type VisibilityState
	} from '@tanstack/table-core';
	import { createSvelteTable, FlexRender } from '$lib/components/ui/data-table/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { TreeRow } from '$lib/components/chart-area/data-table/column';
	import { setContext } from 'svelte';

	type DataTableProps<TData, TValue> = {
		columns: ColumnDef<TData, TValue>[];
		data: TData[];
		getSubRows?: (originalRow: TData) => TData[] | undefined;
		expanded?: ExpandedState;
		rowSelection?: RowSelectionState;
	};

	let {
		data,
		columns,
		getSubRows,
		expanded = $bindable(),
		rowSelection = $bindable(),
	}: DataTableProps<TData, TValue> = $props();

	let sorting = $state<SortingState>([]);
	let columnVisibility = $state<VisibilityState>({});
	let columnFilters = $state<ColumnFiltersState>([]);
	let tableContext = $state({
		primarySamplingRate: null as number | null
	});
	setContext('tableContext', tableContext);

	// --- State for advanced selection ---
	let lastSelectedRowId: string | null = $state(null);

	// --- Helper Functions ---
	function findNodeById(nodes: TreeRow[], id: string): TreeRow | undefined {
		for (const node of nodes) {
			if (node.id === id) return node;
			if (node.subRows) {
				const found = findNodeById(node.subRows, id);
				if (found) return found;
			}
		}
		return undefined;
	}

	function getLeafNodes(node: TreeRow | undefined): TreeRow[] {
		if (!node) return [];
		if (node.type === 'column') return [node];
		if (node.subRows) return node.subRows.flatMap(getLeafNodes);
		return [];
	}

	function canSelectNode(node: TreeRow, primaryRate: number | null): boolean {
		if (primaryRate === null) {
			return true;
		}
		if (node.type === 'column') {
			return node.samplingRate != null && Math.abs(node.samplingRate - primaryRate) < 1e-6;
		}
		if (node.subRows && node.subRows.length > 0) {
			return node.subRows.some((child) => canSelectNode(child, primaryRate));
		}
		return false;
	}

	function handleRowClick(row: Row<TData>, event: MouseEvent) {
		const { shiftKey, ctrlKey, metaKey } = event;

		if (row.getCanExpand()) {
			if (ctrlKey || metaKey) {
				row.toggleSelected();
			} else {
				row.toggleExpanded();
			}
			return;
		}

		if (!row.getCanSelect()) {
			return;
		}

		if (shiftKey && lastSelectedRowId) {
			const rows = table.getRowModel().rows;
			const lastIndex = rows.findIndex((r) => r.id === lastSelectedRowId);
			const currentIndex = rows.findIndex((r) => r.id === row.id);

			if (lastIndex === -1) {
				row.toggleSelected(); // Fallback
				lastSelectedRowId = row.id;
				return;
			}

			const shouldSelect = !row.getIsSelected();
			const start = Math.min(lastIndex, currentIndex);
			const end = Math.max(lastIndex, currentIndex);
			const rangeRows = rows.slice(start, end + 1);

			const newSelection = { ...(rowSelection ?? {}) };
			for (const r of rangeRows) {
				if (!r.getCanExpand() && r.getCanSelect()) {
					if (shouldSelect) {
						newSelection[r.id] = true;
					} else {
						delete newSelection[r.id];
					}
				}
			}
			table.setRowSelection(newSelection);
		} else {
			row.toggleSelected();
			lastSelectedRowId = row.id;
		}
	}

	// --- State and Table Instance ---
	const table = createSvelteTable({
		get data() {
			return data;
		},
		state: {
			get sorting() {
				return sorting;
			},
			get expanded() {
				return expanded ?? {};
			},
			get rowSelection() {
				return rowSelection;
			},
			get columnFilters() {
				return columnFilters;
			},
			columnVisibility
		},
		getRowId: (row) => row.id,
		columns,
		getSubRows: getSubRows,
		getCoreRowModel: getCoreRowModel(),
		getExpandedRowModel: getExpandedRowModel(),
		onExpandedChange: (updater) => {
			expanded = typeof updater === 'function' ? updater(expanded ?? {}) : updater;
		},
		getFilteredRowModel: getFilteredRowModel(),
		getSortedRowModel: getSortedRowModel(),
		onColumnFiltersChange: (updater) => {
			columnFilters = typeof updater === 'function' ? updater(columnFilters) : updater;
		},
		onRowSelectionChange: (updater) => {
			const newSelection = typeof updater === 'function' ? updater(rowSelection ?? {}) : updater;
			const rowsById = table.getCoreRowModel().rowsById;

			const selectedLeafNodes = Object.keys(newSelection)
				.map((id) => rowsById[id])
				.filter((row) => row && !row.getCanExpand())
				.map((row) => row.original as TreeRow);

			if (selectedLeafNodes.length === 0) {
				rowSelection = {};
				return;
			}

			const firstLeafWithRate = selectedLeafNodes.find((node) => node.samplingRate != null);
			const constrainingRate = firstLeafWithRate?.samplingRate ?? null;

			if (constrainingRate === null) {
				rowSelection = {};
				return;
			}

			const validLeafIds = new Set<string>(
				selectedLeafNodes
					.filter(
						(node) =>
							node.samplingRate != null &&
							Math.abs(node.samplingRate - constrainingRate) < 1e-6
					)
					.map((node) => node.id)
			);

			const consistentSelection: RowSelectionState = {};
			const parentIdsToSelect = new Set<string>();

			validLeafIds.forEach((leafId) => {
				consistentSelection[leafId] = true;
				let parent = rowsById[leafId]?.getParentRow();
				while (parent) {
					parentIdsToSelect.add(parent.id);
					parent = parent.getParentRow();
				}
			});

			parentIdsToSelect.forEach((parentId) => {
				consistentSelection[parentId] = true;
			});

			rowSelection = consistentSelection;
		},
		onSortingChange: (updater) => {
			sorting = typeof updater === 'function' ? updater(sorting) : updater;
		},
		enableRowSelection: (row) => {
			return canSelectNode(row.original, tableContext.primarySamplingRate);
		}
	});

	$effect(() => {
		const selectedIds = Object.keys(rowSelection ?? {});
		if (selectedIds.length === 0) {
			tableContext.primarySamplingRate = null;
			return;
		}

		const selectedLeafNodes = selectedIds.flatMap((id) => getLeafNodes(findNodeById(data, id)));
		const selectedRates = selectedLeafNodes
			.map((leaf) => leaf.samplingRate)
			.filter((rate): rate is number => rate != null);

		tableContext.primarySamplingRate = selectedRates.length > 0 ? selectedRates[0] : null;
	});

</script>

<div class="rounded-md border">
	<div class="sticky top-0 z-10 bg-background">
		<Table.Root class="w-full table-fixed">
			<Table.Header class="h-[40px]">
				{#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
					<Table.Row>
						{#each headerGroup.headers as header (header.id)}
							<Table.Head class="bg-muted/50" colspan={header.colSpan} style="width: {header.getSize()}px;">
								{#if !header.isPlaceholder}
									<FlexRender content={header.column.columnDef.header} context={header.getContext()} />
								{/if}
							</Table.Head>
						{/each}
					</Table.Row>
				{/each}
			</Table.Header>
		</Table.Root>
	</div>

	<Table.Root class="w-full table-fixed">
		<Table.Body>
			{#each table.getRowModel().rows as row (row.id)}
				<Table.Row
					data-state={row.getIsSelected() && 'selected'}
					class={row.getCanExpand() || (!row.getCanExpand() && row.getCanSelect())
						? 'cursor-pointer'
						: ''}
					onmousedown={(e) => {
						if (e.shiftKey) {
							e.preventDefault();
						}
					}}
					onclick={(e) => {
						const target = e.target as HTMLElement;
						if (target.closest('[role="checkbox"], button, a')) {
							return;
						}
						handleRowClick(row, e);
					}}
				>
					{#each row.getVisibleCells() as cell (cell.id)}
						<Table.Cell class={cell.column.id === 'name' ? 'truncate' : ''} style="width: {cell.column.getSize()}px;">
							<FlexRender content={cell.column.columnDef.cell} context={cell.getContext()} />
						</Table.Cell>
					{/each}
				</Table.Row>
			{:else}
				<Table.Row>
					<Table.Cell colspan={columns.length} class="h-24 text-center"> No results. </Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>