<script lang="ts" generics="TData extends { id: string }, TValue">
	import {
		type ColumnDef,
		type Row,
		type RowSelectionState,
		type ExpandedState,
		getCoreRowModel,
		getExpandedRowModel
	} from '@tanstack/table-core';
	import { createSvelteTable, FlexRender } from '$lib/components/ui/data-table/index.js';
	import * as Table from '$lib/components/ui/table/index.js';

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
		rowSelection = $bindable()
	}: DataTableProps<TData, TValue> = $props();

	let lastSelectedRowId: string | null = $state(null);

	function getAllDescendantIds(row: Row<TData>): string[] {
		if (!row.subRows || row.subRows.length === 0) {
			return [];
		}
		const ids: string[] = [];
		for (const subRow of row.subRows) {
			ids.push(subRow.id);
			ids.push(...getAllDescendantIds(subRow));
		}
		return ids;
	}

	function handleRowClick(row: Row<TData>, event: MouseEvent) {
		const { shiftKey } = event;

		if (shiftKey && lastSelectedRowId) {
			const flatRows = table.getCoreRowModel().flatRows;
			const lastIndex = flatRows.findIndex((r) => r.id === lastSelectedRowId);
			const currentIndex = row.index;

			if (lastIndex === -1) {
				row.toggleSelected();
				lastSelectedRowId = row.id;
				return;
			}

			const newSelection = { ...(rowSelection ?? {}) };
			const shouldSelect = !row.getIsSelected();
			const start = Math.min(lastIndex, currentIndex);
			const end = Math.max(lastIndex, currentIndex);
			const rangeRows = flatRows.slice(start, end + 1);

			for (const r of rangeRows) {
				const allIdsInBranch = [r.id, ...getAllDescendantIds(r)];
				for (const id of allIdsInBranch) {
					if (shouldSelect) {
						newSelection[id] = true;
					} else {
						delete newSelection[id];
					}
				}
			}
			table.setRowSelection(newSelection);
		} else {
			row.toggleSelected();
		}
		lastSelectedRowId = row.id;
	}

	const table = createSvelteTable({
		get data() {
			return data;
		},
		columns,
		getSubRows,
		getRowId: (row) => row.id,

		state: {
			get expanded() {
				return expanded ?? {};
			},
			get rowSelection() {
				return rowSelection ?? {};
			}
		},


		enableRowSelection: true,
		enableSubRowSelection: true,

		onExpandedChange: (updater) => {
			expanded = typeof updater === 'function' ? updater(expanded ?? {}) : updater;
		},
		onRowSelectionChange: (updater) => {
			rowSelection = typeof updater === 'function' ? updater(rowSelection ?? {}) : updater;
		},

		getCoreRowModel: getCoreRowModel(),
		getExpandedRowModel: getExpandedRowModel()
	});
</script>

<div class="rounded-md border">
	<Table.Root class="w-full table-fixed">
		<Table.Header class="h-[40px] sticky top-0 z-10 bg-muted/50">
			{#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
				<Table.Row>
					{#each headerGroup.headers as header (header.id)}
						<Table.Head colspan={header.colSpan} style="width: {header.getSize()}px;">
							{#if !header.isPlaceholder}
								<FlexRender content={header.column.columnDef.header} context={header.getContext()} />
							{/if}
						</Table.Head>
					{/each}
				</Table.Row>
			{/each}
		</Table.Header>
		<Table.Body>
			{#each table.getRowModel().rows as row (row.id)}
				<Table.Row
					data-state={row.getIsSelected() && 'selected'}
					class="cursor-pointer"
					onmousedown={(e) => {
						if (e.shiftKey) e.preventDefault();
					}}
					onclick={(e) => {
						if ((e.target as HTMLElement).closest('[role="checkbox"], button, a')) {
							return;
						}
						handleRowClick(row, e);
					}}
				>
					{#each row.getVisibleCells() as cell (cell.id)}
						<Table.Cell
							class={cell.column.id === 'name' ? 'truncate' : ''}
							style="width: {cell.column.getSize()}px;"
						>
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