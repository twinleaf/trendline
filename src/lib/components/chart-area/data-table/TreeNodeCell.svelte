<script lang="ts">
    import type { Row } from '@tanstack/table-core';
    import type { TreeRow } from '$lib/components/chart-area/data-table/column';
    import Button from '$lib/components/ui/button/button.svelte';
    import * as Tooltip from "$lib/components/ui/tooltip/index.js";
    import DeviceInfoHoverCard from '$lib/components/device-select/DeviceInfoHoverCard.svelte';
    import { ChevronDown, ChevronRight, Info } from '@lucide/svelte';

    let { row }: { row: Row<TreeRow> } = $props();
    
    const item = row.original;
    const paddingLeft = `${row.depth * 1.25}rem`;
</script>

<div style="padding-left: {paddingLeft}" class="flex items-center gap-2">
    {#if row.getCanExpand()}
        <Button 
            variant="ghost" 
            size="icon" 
            class="h-6 w-6" 
            onclick={row.getToggleExpandedHandler()}
        >
            {#if row.getIsExpanded()}
                <ChevronDown class="size-4" />
            {:else}
                <ChevronRight class="size-4" />
            {/if}
        </Button>
    {:else}
        <span class="inline-block w-4"></span>
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

    {:else} <span>{item.name}</span>
    {/if}
</div>