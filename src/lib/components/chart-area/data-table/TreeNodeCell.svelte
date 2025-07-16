<script lang="ts">
	import type { Row, Table } from '@tanstack/table-core';
    import type { TreeRow } from '$lib/components/chart-area/data-table/column';
    import Button from '$lib/components/ui/button/button.svelte';
    import * as Tooltip from "$lib/components/ui/tooltip/index.js";
    import DeviceInfoHoverCard from '$lib/components/device-select/DeviceInfoHoverCard.svelte';
    import { ChevronDown, ChevronRight, Info, TriangleAlert } from '@lucide/svelte';
    import { getContext } from 'svelte';

    type Props = {
            row: Row<TreeRow>;
        };
    let { row }: Props = $props();    
    const item = row.original;
    const paddingLeft = `${row.depth * 1.25}rem`;

    const tableContext = getContext<{ primarySamplingRate: number | null }>('tableContext');

	let primaryRate = $derived(tableContext.primarySamplingRate);
	let isMismatched = $derived(
		item.type === 'stream' &&
		primaryRate != null &&
		item.samplingRate != null &&
		Math.abs(item.samplingRate - primaryRate) > 1e-6
	);
    $inspect(isMismatched);
</script>

<div style="padding-left: {paddingLeft}" class="flex items-center gap-2">
	{#if row.getCanExpand()}
		<Button variant="ghost" size="icon" class="h-6 w-6" onclick={row.getToggleExpandedHandler()}>
			{#if row.getIsExpanded()}
				<ChevronDown class="size-4" />
			{:else}
				<ChevronRight class="size-4" />
			{/if}
		</Button>
	{:else}
		<span class="inline-block w-6"></span>
	{/if}

	{#if isMismatched}
		<Tooltip.Provider delayDuration={ 500 }>
			<Tooltip.Root>
				<Tooltip.Trigger>
					<TriangleAlert class="size-4 text-amber-500" />
				</Tooltip.Trigger>
				<Tooltip.Content>
					<p>
						Rate ({item.samplingRate?.toFixed(2)} Hz) mismatches selection ({primaryRate?.toFixed(2)} Hz).
					</p>
				</Tooltip.Content>
			</Tooltip.Root>
		</Tooltip.Provider>
	{/if}

	{#if item.type === 'device'}
		<span class="font-semibold">{item.name}</span>
		{#if item.device}
			<DeviceInfoHoverCard device={item.device} />
		{/if}
	{:else if item.type === 'column'}
		<span>{item.name}</span>
		{#if item.description}
			<Tooltip.Provider>
				<Tooltip.Root>
					<Tooltip.Trigger class="cursor-default">
						<Info class="size-3.5 text-muted-foreground" />
					</Tooltip.Trigger>
					<Tooltip.Content side="top">
						<p class="max-w-xs">{item.description}</p>
					</Tooltip.Content>
				</Tooltip.Root>
			</Tooltip.Provider>
		{/if}
	{:else}
		<span>{item.name}</span>
	{/if}
</div>