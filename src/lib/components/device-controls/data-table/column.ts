import type { ColumnDef } from "@tanstack/table-core";
import type { UiDevice } from '$lib/bindings/UiDevice';
import type { RpcMeta } from '$lib/bindings/RpcMeta';
import { renderComponent } from "$lib/components/ui/data-table";
import RpcActionCell from "./RpcActionCell.svelte";


export type RpcTableMeta = {
    device: UiDevice;
}

export const columns: ColumnDef<RpcMeta>[] = [
  { 
    accessorKey: 'name', 
    header: 'Name', 
    size: 200 
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
    size: 100 
  },
  { 
    accessorKey: 'permissions', 
    header: 'Perms', 
    size: 80 
  }
];