<script lang="ts" generics="TData extends { name: string }, TValue">
    import { type ColumnDef, getCoreRowModel } from "@tanstack/table-core";
    import { createSvelteTable, FlexRender } from "$lib/components/ui/data-table/index.js";
    import * as Table from "$lib/components/ui/table/index.js";
	import type { UiDevice } from "$lib/bindings/UiDevice";
    import { Funnel, ChevronDown } from "@lucide/svelte";
    import { Input } from "$lib/components/ui/input";
    import { Checkbox } from "$lib/components/ui/checkbox";
    import { Label } from "$lib/components/ui/label";
    import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
    import Button from "$lib/components/ui/button/button.svelte";

    type DataTableProps = {
        columns: ColumnDef<TData, TValue>[];
        data: TData[];
        device: UiDevice;
    };

    let { data, columns, device }: DataTableProps = $props();

    let isHeaderVisible = $state(true);

    const table = createSvelteTable({
        get data() { return data; },
        get columns() { return columns; },
        meta: {
            get device() { return device; }
        },
        getCoreRowModel: getCoreRowModel(),
    });
</script>

<div class="rounded-md border overflow-y-auto container max-h-96">
	<div class="sticky top-0 z-10 bg-background">
		<div class="flex items-center p-4 border-b gap-4">
			<Input
				placeholder="Filter RPCs..."
				value={(table.getColumn('email')?.getFilterValue() as string) ?? ''}
				oninput={(e) => table.getColumn('email')?.setFilterValue(e.currentTarget.value)}
				onchange={(e) => {
					table.getColumn('email')?.setFilterValue(e.currentTarget.value);
				}}
				class="max-w-sm"
			/>
			<div class="ml-auto flex items-center gap-4">
				<div class="flex items-center space-x-2">
					<Checkbox id="show-header" bind:checked={isHeaderVisible} />
					<Label for="show-header" class="text-sm font-medium">Header</Label>
				</div>

				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						{#snippet child({ props })}
						<Button {...props} variant="outline" class="h-9">
							<Funnel class="size-4" />
							<ChevronDown class="ml-2 size-4" />
						</Button>
						{/snippet}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end">
						{#each table.getAllColumns().filter((col) => col.getCanHide()) as column (column)}
							<DropdownMenu.CheckboxItem
								class="capitalize"
								bind:checked={() => column.getIsVisible(), (v) => column.toggleVisibility(!!v)}
							>
								{column.id}
							</DropdownMenu.CheckboxItem>
						{/each}
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			</div>
		</div>

		{#if isHeaderVisible}
			<Table.Root class="w-full table-fixed">
				<Table.Header>
					{#each table.getHeaderGroups() as headerGroup (headerGroup.id)}
						<Table.Row>
							{#each headerGroup.headers as header (header.id)}
								<Table.Head class="bg-muted/50" style={`width: ${header.getSize()}px`}>
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
		{/if}
	</div>

	<Table.Root class="w-full table-fixed">
		<Table.Body>
			{#if table.getRowModel().rows.length}
				{#each table.getRowModel().rows as row (row.id)}
					<Table.Row>
						{#each row.getVisibleCells() as cell (cell.id)}
							<Table.Cell style={`width: ${cell.column.getSize()}px`}>
								<FlexRender content={cell.column.columnDef.cell} context={cell.getContext()} />
							</Table.Cell>
						{/each}
					</Table.Row>
				{/each}
			{:else}
				<Table.Row>
					<Table.Cell colspan={columns.length} class="h-24 text-center">
						No RPCs available for this device.
					</Table.Cell>
				</Table.Row>
			{/if}
		</Table.Body>
	</Table.Root>
</div>