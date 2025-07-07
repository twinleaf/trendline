<script lang="ts" generics="TData extends TreeRow, TValue">
    import { 
        type ColumnDef, 
        type RowSelectionState,
        type ExpandedState,
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

    let { data, columns, getSubRows, initialExpanded, rowSelection = $bindable() }: DataTableProps<TData, TValue> = $props();
    let expanded = $state<ExpandedState>(initialExpanded ?? {}); 

    const table = createSvelteTable({
        get data() {
            return data;
        },
        state: {
            get expanded() { return expanded; },
            get rowSelection() { return rowSelection; },
        },
        getRowId: (row) => row.id,
        columns,
        getSubRows: getSubRows,
        getCoreRowModel: getCoreRowModel(),
        getExpandedRowModel: getExpandedRowModel(),
        onExpandedChange: (updater) => {
            expanded = typeof updater === 'function' ? updater(expanded) : updater;
        },
        enableRowSelection: true,
        onRowSelectionChange: (updater) => {
            rowSelection = typeof updater === 'function' 
                ? updater(rowSelection ?? {}) 
                : updater;
        },
    });
</script>

<div class="rounded-md border">
    <Table.Root>
        <Table.Header>
            {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
                <Table.Row>
                    {#each headerGroup.headers as header (header.id)}
                        <Table.Head colspan={header.colSpan}>
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
        <Table.Body>
            {#each table.getRowModel().rows as row (row.id)}
                <Table.Row data-state={row.getIsSelected() && "selected"}>
                    {#each row.getVisibleCells() as cell (cell.id)}
                        <Table.Cell>
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