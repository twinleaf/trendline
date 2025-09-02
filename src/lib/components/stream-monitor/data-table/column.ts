import type { ColumnDef } from '@tanstack/table-core';
import type { TreeRow } from '$lib/components/chart-area/data-table/column';
import { renderComponent } from '$lib/components/ui/data-table';
import { Checkbox } from '$lib/components/ui/checkbox';
import TreeNodeCell from './TreeNodeCell.svelte';

export const columns: ColumnDef<TreeRow>[] = [
	{
		id: 'select',
		header: ({ table }) =>
			renderComponent(Checkbox, {
				checked: table.getIsAllPageRowsSelected(),
				indeterminate: table.getIsSomeRowsSelected(),
				onCheckedChange: (value) => table.toggleAllPageRowsSelected(!!value)
			}),
		cell: ({ row }) => {
			return renderComponent(Checkbox, {
				checked: row.getIsSelected(),
				indeterminate: row.getIsSomeSelected(),
				onCheckedChange: (value) => row.toggleSelected(!!value)
			});
		},
		size: 40
	},
	{
		id: 'name',
		header: 'Name',
		cell: ({ row }) => renderComponent(TreeNodeCell, { row }),
		size: 200
	},
	{
		accessorKey: 'units',
		header: 'Units',
		cell: ({ row }) => (row.original.type === 'column' ? row.original.units : ''),
		size: 100
	}
];