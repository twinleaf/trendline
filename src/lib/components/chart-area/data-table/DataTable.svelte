<script lang="ts" generics="TData extends TreeRow, TValue">
    import { 
        type ColumnDef, 
        type RowSelectionState,
        type ExpandedState,
        type ColumnFiltersState,
        getCoreRowModel,
        getExpandedRowModel,
        getFilteredRowModel,
        getSortedRowModel,
    } from "@tanstack/table-core";
    import {
        createSvelteTable,
        FlexRender,
        } from "$lib/components/ui/data-table/index.js";
    import * as Table from "$lib/components/ui/table/index.js";
    import type { TreeRow } from "$lib/components/chart-area/data-table/column";

    type DataTableProps<TData, TValue> = {
        columns: ColumnDef<TData, TValue>[];
        data: TData[];
        getSubRows?: (originalRow: TData) => TData[] | undefined;
        initialExpanded?: ExpandedState;
        rowSelection?: RowSelectionState;
    };

    let {
		data,
		columns,
		getSubRows,
		initialExpanded,
		rowSelection = $bindable()
	}: DataTableProps<TData, TValue> = $props();
    let expanded = $state<ExpandedState>(initialExpanded ?? {}); 
    let columnFilters = $state<ColumnFiltersState>([]);
    
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
    
    // --- State and Table Instance ---
    const table = createSvelteTable({
        get data() { return data; },
        state: {
            get expanded() { return expanded; },
            get rowSelection() { return rowSelection; },
            get columnFilters() { return columnFilters; },
        },
        getRowId: (row) => row.id,
        columns,
        getSubRows: getSubRows,
        getCoreRowModel: getCoreRowModel(),
        getExpandedRowModel: getExpandedRowModel(),
        onExpandedChange: (updater) => {
            expanded = typeof updater === 'function' ? updater(expanded) : updater;
        },
        getFilteredRowModel: getFilteredRowModel(),
        onColumnFiltersChange: (updater) => {
            columnFilters = typeof updater === 'function' ? updater(columnFilters) : updater;
        },
        onRowSelectionChange: (updater) => {
			rowSelection = typeof updater === 'function' ? updater(rowSelection ?? {}) : updater;
		},
        enableRowSelection: true,
        filterFromLeafRows: true, 
    });

    // --- Core Reactivity Logic ---
    let previousUniqueRates = new Set<number>();

    $effect(() => {
        const selectedIds = Object.keys(rowSelection ?? {});

        const selectedLeafNodes = selectedIds.flatMap(id => getLeafNodes(findNodeById(data, id)));
        const currentUniqueRates = new Set(selectedLeafNodes.map(leaf => leaf.samplingRate).filter(rate => rate != null));
        
        let newFilterValue: number | null = null;
        if (currentUniqueRates.size > 0) {
            newFilterValue = currentUniqueRates.values().next().value ?? null;
        }

        if (currentUniqueRates.size > 1 && previousUniqueRates.size <= 1) {
            console.error("Conflict: Multiple sample rates selected. The plot will only display data for the first selected rate.");
        }

        previousUniqueRates = currentUniqueRates;

        const currentFilterValue = table.getColumn('select')?.getFilterValue();
        if (currentFilterValue !== newFilterValue) {
            table.getColumn('select')?.setFilterValue(newFilterValue);
        }
    });

</script>

<div class="h-full rounded-md border overflow-y-auto">
    <div class="sticky top-0 bg-background z-10">
        <Table.Root class="w-full table-fixed">
            <Table.Header class="h-[40px]">
                {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
                    <Table.Row>
                        {#each headerGroup.headers as header (header.id)}
                            <Table.Head
                                class="bg-muted/50"
                                colspan={header.colSpan}
                                style="width: {header.getSize()}px;"
                            >
                                {#if !header.isPlaceholder}
                                    <FlexRender
                                    content={header.column.columnDef.header}
                                    context={header.getContext()}
                                    />
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
                <Table.Row data-state={row.getIsSelected() && "selected"}>
                    {#each row.getVisibleCells() as cell (cell.id)}
                        <Table.Cell
							class={cell.column.id === 'name' ? 'truncate' : ''}
							style="width: {cell.column.getSize()}px;"
						>
                            <FlexRender
                                content={cell.column.columnDef.cell}
                                context={cell.getContext()}
                            />
                        </Table.Cell>
                        {/each}
                </Table.Row>
            {:else}
                <Table.Row>
                    <Table.Cell colspan={columns.length} class="h-24 text-center">
                    No results.
                    </Table.Cell>
                </Table.Row>
            {/each}
        </Table.Body>
    </Table.Root>
</div>