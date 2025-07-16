import type { ColumnDef } from "@tanstack/table-core";
import type { UiDevice } from '$lib/bindings/UiDevice';
import type { RpcMeta } from '$lib/bindings/RpcMeta';
import { renderComponent } from "$lib/components/ui/data-table";
import RpcActionCell from "./RpcActionCell.svelte";
import DataTableSortHeader from "./DataTableSortHeader.svelte";
import { fuzzyFilter, fuzzySort, prefixFilter } from './filtering';


export type RpcTableMeta = {
    device: UiDevice;
}

export const columns: ColumnDef<RpcMeta>[] = [
  { 
    accessorKey: 'name', 
    header: ({ column }) => {
        return renderComponent(DataTableSortHeader, { title: 'Name', column });
    },
    filterFn: fuzzyFilter,
    sortingFn: fuzzySort,
    size: 130 
  },
  {
    id: 'name_prefix',
    accessorFn: (row) => row.name.split('.')[0],
    filterFn: prefixFilter,
    enableSorting: false,
  },
  {
    id: 'action',
    header: 'Value / Action',
    cell: ({ row, table }) => {
        const rpc = row.original;
        const { device } = table.options.meta as RpcTableMeta;
        return renderComponent(RpcActionCell, { rpc, device });
    },
  },
  { 
    accessorKey: 'arg_type', 
    header: 'Type', 
    size: 40,
    enableHiding: true
  },
  { 
    accessorKey: 'permissions', 
    header: ({ column }) => {
        return renderComponent(DataTableSortHeader, { title: 'Perms', column });
    },
    size: 80,
    enableHiding: true 
  }
];