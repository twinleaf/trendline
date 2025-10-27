<script lang="ts">
  import type { DataColumnId } from '$lib/bindings/DataColumnId';
  import type { UiDevice } from '$lib/bindings/UiDevice';
  import type { UiStream } from '$lib/bindings/UiStream';
  import type { ColumnMeta } from '$lib/bindings/ColumnMeta';
  import { deviceState } from '$lib/states/deviceState.svelte';
  import { useNodes, Handle, Position, type Node, type NodeProps } from '@xyflow/svelte';
  import CheckIcon from "@lucide/svelte/icons/check";
  import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
  import InfoIcon from "@lucide/svelte/icons/info";
  import { tick } from "svelte";
  import * as Command from "$lib/components/ui/command/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import * as HoverCard from "$lib/components/ui/hover-card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { cn } from "$lib/utils.js";

  let { id, data }: NodeProps<Node<{ label?: string; key?: DataColumnId }>> = $props();

  const nodes = useNodes();

  // Build grouped options by device
  type Option = { 
    deviceName: string;
    deviceRoute: string;
    streamName: string; 
    columnName: string; 
    label: string; 
    key: DataColumnId;
    searchValue: string;
    uniqueId: string; // Add unique identifier for command items
  };
  
  type GroupedOptions = {
    deviceName: string;
    deviceRoute: string;
    options: Option[];
  };

  let groupedOptions = $derived.by((): GroupedOptions[] => {
    const deviceMap = new Map<string, GroupedOptions>();
    
    for (const dev of deviceState.selectedDevices as UiDevice[]) {
      const deviceKey = `${dev.route}`;
      
      if (!deviceMap.has(deviceKey)) {
        deviceMap.set(deviceKey, {
          deviceName: dev.meta.name,
          deviceRoute: dev.route,
          options: []
        });
      }
      
      const group = deviceMap.get(deviceKey)!;
      
      for (const s of dev.streams as UiStream[]) {
        for (const c of s.columns as ColumnMeta[]) {
          const key: DataColumnId = {
            port_url: dev.url, 
            device_route: dev.route,
            stream_id: s.meta.stream_id, 
            column_index: c.index
          };
          
          const label = `${s.meta.name} • ${c.name}`;
          const uniqueId = JSON.stringify(key); // Use serialized key as unique ID
          
          group.options.push({ 
            deviceName: dev.meta.name,
            deviceRoute: dev.route,
            streamName: s.meta.name,
            columnName: c.name,
            label,
            key,
            searchValue: `${dev.meta.name} ${s.meta.name} ${c.name}`.toLowerCase(),
            uniqueId
          });
        }
      }
    }
    
    // Sort groups by route, then sort options within each group
    return Array.from(deviceMap.values())
      .sort((a, b) => a.deviceRoute.localeCompare(b.deviceRoute))
      .map(group => ({
        ...group,
        options: group.options.sort((a, b) => a.label.localeCompare(b.label))
      }));
  });

  // Flatten all options for easier searching
  let allOptions = $derived(groupedOptions.flatMap(g => g.options));

  function setKey(selectedKey: DataColumnId | null) {
    if (selectedKey) {
      nodes.update(ns => ns.map(n => n.id === id ? { ...n, data: { ...n.data, key: selectedKey } } : n));
    }
  }

  let open = $state(false);
  let value = $state("");
  let triggerRef = $state<HTMLButtonElement>(null!);

  const selectedOption = $derived(
    allOptions.find((opt) => JSON.stringify(opt.key) === value)
  );
  
  const selectedValue = $derived(
    selectedOption ? `${selectedOption.deviceName} • ${selectedOption.label}` : null
  );

  // Update value when data.key changes
  $effect(() => {
    if (data.key) {
      value = JSON.stringify(data.key);
    }
  });

  function closeAndFocusTrigger() {
    open = false;
    tick().then(() => {
      triggerRef?.focus();
    });
  }
</script>

<div class="rounded-md border bg-card text-card-foreground shadow p-3 w-[280px]">
  <div class="flex items-center justify-between mb-3">
    <div class="text-xs font-semibold">Input</div>
    {#if data.key}
      <HoverCard.Root>
        <HoverCard.Trigger>
          <InfoIcon class="h-3 w-3 opacity-60 hover:opacity-100 transition-opacity" />
        </HoverCard.Trigger>
        <HoverCard.Content class="w-80">
          <div class="space-y-1">
            <h4 class="text-xs font-semibold">Column Key</h4>
            <pre class="text-[10px] bg-muted p-2 rounded overflow-auto">{JSON.stringify(data.key, null, 2)}</pre>
          </div>
        </HoverCard.Content>
      </HoverCard.Root>
    {/if}
  </div>

  <Popover.Root bind:open>
    <Popover.Trigger bind:ref={triggerRef}>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="outline"
          class="w-full justify-between text-xs h-8 nodrag"
          role="combobox"
          aria-expanded={open}
          onpointerdown={(e) => e.stopPropagation()}
        >
          <span class="truncate">{selectedValue || "Select column..."}</span>
          <ChevronsUpDownIcon class="ml-2 h-3 w-3 shrink-0 opacity-50" />
        </Button>
      {/snippet}
    </Popover.Trigger>
    <Popover.Content class="w-[350px] p-0">
      <Command.Root>
        <Command.Input placeholder="Search column..." class="h-8 text-xs" />
        <Command.List>
          <Command.Empty class="text-xs">No column found.</Command.Empty>
          {#each groupedOptions as group (group.deviceRoute)}
            <Command.Group>
              <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground">
                {group.deviceName} ({group.deviceRoute})
              </div>
              {#each group.options as opt (opt.uniqueId)}
                <Command.Item
                  value={opt.uniqueId}
                  onSelect={() => {
                    value = JSON.stringify(opt.key);
                    setKey(opt.key);
                    closeAndFocusTrigger();
                  }}
                  class="text-xs"
                >
                  <CheckIcon
                    class={cn("mr-2 h-3 w-3", value !== JSON.stringify(opt.key) && "text-transparent")}
                  />
                  {opt.label}
                </Command.Item>
              {/each}
            </Command.Group>
          {/each}
        </Command.List>
      </Command.Root>
    </Popover.Content>
  </Popover.Root>

  <div class="mt-3 pt-3 border-t">
    <div class="text-[10px] text-muted-foreground text-center mb-1">Output</div>
    <div class="flex justify-center h-3">
      <Handle type="source" position={Position.Bottom} />
    </div>
  </div>
</div>