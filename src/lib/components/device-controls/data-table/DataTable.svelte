<script lang="ts" generics="TData extends { name: string }, TValue">
    import {
		type ColumnDef,
		getCoreRowModel,
		getSortedRowModel,
		getFilteredRowModel,
		type SortingState,
		type ColumnFiltersState,
	} from '@tanstack/table-core';
    import { createSvelteTable, FlexRender } from "$lib/components/ui/data-table/index.js";
    import * as Table from "$lib/components/ui/table/index.js";
	import type { UiDevice } from "$lib/bindings/UiDevice";
    import { Input } from "$lib/components/ui/input";
    import { Checkbox } from "$lib/components/ui/checkbox";
    import { Label } from "$lib/components/ui/label";
    import DropDownFilter from '$lib/components/device-controls/data-table/DropDownFilter.svelte';
    import { SvelteSet } from 'svelte/reactivity';
    import { onMount } from 'svelte';


    type DataTableProps = {
		columns: ColumnDef<TData, TValue>[];
		data: TData[];
		device: UiDevice;
	};
	let { data, columns, device }: DataTableProps = $props();

	// --- State Management ---
	let isHeaderVisible = $state(true);
	let sorting = $state<SortingState>([{ id: 'name', desc: false }]);
	let fuzzySearchValue = $state('');
	
	// --- State for the Dropdown ---
	let allPrefixes = $state<string[]>([]);
	let selectedPrefixes = $state(new SvelteSet<string>());

	// --- Derived State for TanStack Table ---
	const columnFilters = $derived([
		{ id: 'name', value: fuzzySearchValue },
		{ id: 'name_prefix', value: [...selectedPrefixes] }
	]);

	// --- Table Instance ---
	const table = createSvelteTable({
		get data() { return data; },
		get columns() { return columns; },
		meta: { get device() { return device; } },
		state: {
			get sorting() { return sorting; },
			get columnFilters() { return columnFilters; }
		},
		onSortingChange: (updater) => (sorting = typeof updater === 'function' ? updater(sorting) : updater),
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getFilteredRowModel: getFilteredRowModel()
	});

    onMount(() => {
        console.log('[DataTable] onMount fired. Setting up filters from data prop.');

        const calculatedPrefixes = [...new Set(data.map((rpc) => rpc.name.split('.')[0]))].sort();
        allPrefixes = calculatedPrefixes;

        const initialSelection = new SvelteSet(
            calculatedPrefixes.filter(p => p !== 'rpc' && p !== 'dev')
        );
        selectedPrefixes = initialSelection;
    });

    $inspect(columnFilters);

</script> 

<div class="rounded-md border">
    <div class="sticky top-0 z-10 bg-background">
        <div class="flex items-center p-4 border-b gap-4">
            <Input
                placeholder="Search RPCs..."
                bind:value={fuzzySearchValue}
                class="max-w-sm"
            />
            <div class="ml-auto flex items-center gap-4">
                <div class="flex items-center space-x-2">
                    <Checkbox id="show-header" bind:checked={isHeaderVisible} />
                    <Label for="show-header" class="text-sm font-medium">Header</Label>
                </div>
                <DropDownFilter 
                    {allPrefixes} 
                    bind:selected={selectedPrefixes} 
                />
            </div>
        </div>

        {#if isHeaderVisible}
            <Table.Root class="w-full table-fixed">
                <Table.Header>
                    {#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
                        <Table.Row>
                            {#each headerGroup.headers as header (header.id)}
                                {#if header.column.id !== 'name_prefix'}
                                    <Table.Head class="bg-muted/50" style={`width: ${header.getSize()}px`}>
                                        {#if !header.isPlaceholder}
                                            <FlexRender
                                                content={header.column.columnDef.header}
                                                context={header.getContext()}
                                            />
                                        {/if}
                                    </Table.Head>
                                {/if}
                            {/each}
                        </Table.Row>
                    {/each}
                </Table.Header>
            </Table.Root>
        {/if}
    </div>

    <Table.Root class="w-full table-fixed">
        <Table.Body>
            {#if table.getRowModel().rows.length}
                {#each table.getRowModel().rows as row (row.id)}
                    <Table.Row>
                        {#each row.getVisibleCells() as cell (cell.id)}
                            {#if cell.column.id !== 'name_prefix'}
                                <Table.Cell style={`width: ${cell.column.getSize()}px`}>
                                    <FlexRender content={cell.column.columnDef.cell} context={cell.getContext()} />
                                </Table.Cell>
                            {/if}
                        {/each}
                    </Table.Row>
                {/each}
            {:else}
                <Table.Row>
                    <Table.Cell colspan={columns.length - 1} class="h-24 text-center">
                        No results found.
                    </Table.Cell>
                </Table.Row>
            {/if}
        </Table.Body>
    </Table.Root>
</div>