<script lang="ts" generics="TData extends { name: string }, TValue">
	import {
		type ColumnDef,
		getCoreRowModel,
		getSortedRowModel,
		getFilteredRowModel,
		type SortingState,
		type VisibilityState
	} from '@tanstack/table-core';
	import { createSvelteTable, FlexRender } from '$lib/components/ui/data-table/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	import { Input } from '$lib/components/ui/input';
	import DropDownFilter from '$lib/components/device-controls/data-table/DropDownFilter.svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { onMount } from 'svelte';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { ChevronDown, ChevronUp, FileType2, UserPen } from '@lucide/svelte';
    import { cn } from '$lib/utils';

	type DataTableProps = {
		columns: ColumnDef<TData, TValue>[];
		data: TData[];
		device: UiDevice;
	};
	let { data, columns, device }: DataTableProps = $props();

	// --- State Management ---
	let sorting = $state<SortingState>([{ id: 'name', desc: false }]);
	let fuzzySearchValue = $state('');
	let toggleState = $state<string[]>(['header']); // Default toggles

	// --- State for the Dropdown ---
	let allPrefixes = $state<string[]>([]);
	let selectedPrefixes = $state(new SvelteSet<string>());

	// --- Derived State for TanStack Table ---
	const isHeaderVisible = $derived(toggleState.includes('header'));
	const columnVisibility = $derived<VisibilityState>({
		arg_type: toggleState.includes('type'),
		permissions: toggleState.includes('perms')
	});
	const columnFilters = $derived([
		{ id: 'name', value: fuzzySearchValue },
		{ id: 'name_prefix', value: [...selectedPrefixes] }
	]);

	// --- Table Instance ---
	const table = createSvelteTable({
		get data() {
			return data;
		},
		get columns() {
			return columns;
		},
		meta: {
			get device() {
				return device;
			}
		},
		state: {
			get sorting() {
				return sorting;
			},
			get columnFilters() {
				return columnFilters;
			},
			get columnVisibility() {
				return columnVisibility;
			}
		},
		onSortingChange: (updater) =>
			(sorting = typeof updater === 'function' ? updater(sorting) : updater),
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getFilteredRowModel: getFilteredRowModel()
	});

	onMount(() => {
		const calculatedPrefixes = [...new Set(data.map((rpc) => rpc.name.split('.')[0]))].sort();
		allPrefixes = calculatedPrefixes;

		const initialSelection = new SvelteSet(
			calculatedPrefixes.filter((p) => p !== 'rpc' && p !== 'dev')
		);
		selectedPrefixes = initialSelection;
	});
</script>

<div class="rounded-md border h-full overflow-y-auto">
	<div class="sticky top-0 z-10 bg-background">
		<div class="flex items-center p-4 border-b gap-4">
			<Input
				placeholder="Search RPCs..."
				bind:value={fuzzySearchValue}
				class="max-w-sm"
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
				spellcheck="false"
			/>
			<div class="ml-auto flex items-center gap-4">
				<ToggleGroup.Root
					type="multiple"
					class="flex items-center gap-1"
					bind:value={toggleState}
				>
					<Tooltip.Provider delayDuration={ 500 }>
						<Tooltip.Root>
							<Tooltip.Trigger>
								<ToggleGroup.Item
									value="header"
									aria-label="Toggle Header"
								>
									{#if isHeaderVisible}
										<ChevronDown class="size-4" />
									{:else}
										<ChevronUp class="size-4" />
									{/if}
								</ToggleGroup.Item>
							</Tooltip.Trigger>
							<Tooltip.Content>
								<p>Toggle Header</p>
							</Tooltip.Content>
						</Tooltip.Root>
						<Tooltip.Root>
							<Tooltip.Trigger>
								<ToggleGroup.Item value="type" aria-label="Toggle Type Column">
									<FileType2 class="size-4" />
								</ToggleGroup.Item>
							</Tooltip.Trigger>
							<Tooltip.Content>
								<p>Toggle Type Column</p>
							</Tooltip.Content>
						</Tooltip.Root>
						<Tooltip.Root>
							<Tooltip.Trigger>
								<ToggleGroup.Item
									value="perms"
									aria-label="Toggle Permissions Column"
								>
									<UserPen class="size-4" />
								</ToggleGroup.Item>
							</Tooltip.Trigger>
							<Tooltip.Content>
								<p>Toggle Permissions Column</p>
							</Tooltip.Content>
						</Tooltip.Root>
					</Tooltip.Provider>
				</ToggleGroup.Root>

				<DropDownFilter {allPrefixes} bind:selected={selectedPrefixes} />
			</div>
		</div>

		{#if isHeaderVisible}
			<Table.Root class="w-full table-fixed">
				<Table.Header>
					{#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
						<Table.Row>
                            {#each headerGroup.headers as header (header.id)}
                                {#if header.column.id !== 'name_prefix'}
                                    <Table.Head
                                        class={cn('bg-muted/50', {
                                            'text-center':
                                                header.column.id === 'arg_type' || header.column.id === 'permissions'
                                        })}
                                        style={`width: ${header.getSize()}px`}
                                    >
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
								<Table.Cell
									class={cn({
										'text-center':
											cell.column.id === 'arg_type' || cell.column.id === 'permissions'
									})}
									style={`width: ${cell.column.getSize()}px`}
								>
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