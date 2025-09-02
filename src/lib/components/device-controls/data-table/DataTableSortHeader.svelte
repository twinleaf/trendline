<script lang="ts">
	import type { Column } from '@tanstack/table-core';
	import type { RpcMeta } from '$lib/bindings/RpcMeta';
	import { Button } from '$lib/components/ui/button';
	import { ChevronsUpDown, ArrowUp, ArrowDown } from '@lucide/svelte';
	import { cn } from '$lib/utils';

	let { column, title, class: className }: { column: Column<RpcMeta>; title: string; class?: string; } =
			$props();
	const sorted = $derived(column.getIsSorted());
</script>

<Button
	variant="ghost"
	onclick={() => column.toggleSorting(sorted === 'asc')}
	class={cn('w-full h-auto p-1 flex items-start', className)}
>
	{title}
	{#if sorted === 'desc'}
		<ArrowDown class="ml-2 h-4 w-4 shrink-0" />
	{:else if sorted === 'asc'}
		<ArrowUp class="ml-2 h-4 w-4 shrink-0" />
	{:else}
		<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0" />
	{/if}
</Button>