<script lang="ts">
	import type { Row } from '@tanstack/table-core';
	import type { TreeRow } from '$lib/components/chart-area/data-table/column';
	import { ChevronDown, ChevronRight } from '@lucide/svelte';

	type Props = {
		row: Row<TreeRow>;
	};
	let { row }: Props = $props();
</script>

<div class="flex items-center" style:padding-left="{row.depth * 1.5}rem">
	{#if row.getCanExpand()}
		<button
			class="flex items-center justify-center w-6 h-6 -ml-2 mr-1 rounded-sm hover:bg-muted"
			onclick={row.getToggleExpandedHandler()}
		>
			{#if row.getIsExpanded()}
				<ChevronDown class="size-4" />
			{:else}
				<ChevronRight class="size-4" />
			{/if}
		</button>
	{/if}
	<span class="truncate">{row.original.name}</span>
</div>