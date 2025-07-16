import type { ColumnDef } from '@tanstack/table-core';
import type { DataColumnId } from '$lib/bindings/DataColumnId';
import type { UiDevice } from '$lib/bindings/UiDevice';
import { renderComponent } from '$lib/components/ui/data-table';
import { Checkbox } from '$lib/components/ui/checkbox';
import TreeNodeCell from '$lib/components/chart-area/data-table/TreeNodeCell.svelte';

export type TreeRow = {
	id: string;
	type: 'device' | 'stream' | 'column';
	name: string;

	device?: UiDevice; // Only for 'device' rows, for the hover card
	units?: string; // Only for 'column' rows
	dataKey?: DataColumnId; // Only for 'column' rows (the selectable leaves)
	description?: string; // Only for 'column' rows
	samplingRate?: number; // Only for 'stream' and 'column' rows

	subRows?: TreeRow[];
};

export const columns: ColumnDef<TreeRow>[] = [
	{
		id: 'select',
		header: ({ table }) =>
			renderComponent(Checkbox, {
				checked: table.getIsAllPageRowsSelected(),
				onCheckedChange: (value) => table.toggleAllPageRowsSelected(!!value)
			}),
		cell: ({ row }) => {
			return renderComponent(Checkbox, {
				indeterminate: row.getIsSomeSelected(),
				checked: row.getIsSelected(),
				onCheckedChange: (value) => row.toggleSelected(!!value),
				disabled: !row.getCanSelect()
			});
		},
		size: 40,
	},
	{
		id: 'name',
		header: 'Name',
		cell: ({ row }) => renderComponent(TreeNodeCell, { row }),
		size: 350
	},
	{
		accessorKey: 'units',
		header: 'Units',
		cell: ({ row }) => (row.original.type === 'column' ? row.original.units : ''),
		size: 100
	}
];