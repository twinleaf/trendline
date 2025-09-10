<script lang="ts">
  import * as RadioGroup from '$lib/components/ui/radio-group';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Collapsible from '$lib/components/ui/collapsible';
  import { Label } from '$lib/components/ui/label';
  import { ChevronsUpDown, RefreshCw } from '@lucide/svelte';
  import { LoaderCircleIcon } from '@lucide/svelte/icons';
  import DeviceInfoHoverCard from '$lib/components/device-select/DeviceInfoHoverCard.svelte';
  import type { UiDevice } from '$lib/bindings/UiDevice';
  import { deviceState } from '$lib/states/deviceState.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { deviceDomId } from '$lib/utils';

  type DeviceWithChildren = UiDevice & { children: UiDevice[] };

  let {
    devices,
    selectedParent = $bindable(),
  }: {
    devices: DeviceWithChildren[];
    selectedParent: string;
  } = $props();

  async function refreshParent(url: string) {
    try {
      await invoke('refresh_port', { portUrl: url });
    } catch (e) {
      console.error('refresh_port failed', e);
    }
  }
</script>

<RadioGroup.Root
  bind:value={selectedParent}
  orientation="vertical"
  class="relative focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-md"
>
  {#each devices as root (root.url)}
    <Collapsible.Root class="w-full" aria-busy={root.state === 'Discovery' || root.state === 'Reconnecting'}>
      <div class="flex items-center justify-between px-2 py-1.5">
        <div class="flex items-center space-x-2">
          <RadioGroup.Item id={root.url} value={root.url} />
          <Label for={root.url} class="font-medium cursor-pointer">
            {root.meta.name}
          </Label>

          {#if root.state === 'Discovery' || root.state === 'Reconnecting'}
            <span title="Discovering devices">
              <LoaderCircleIcon class="size-4 animate-spin text-zinc-400" aria-label="Discovering devices" />
            </span>
          {:else}
            <button
              type="button"
              class="inline-grid place-items-center p-1 rounded-md hover:bg-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-label="Rescan for children"
              onclick={() => refreshParent(root.url)}
            >
              <RefreshCw class="h-4 w-4 text-zinc-500" />
            </button>
          {/if}

          <DeviceInfoHoverCard device={root} />
        </div>

        {#if root.children.length}
          <Collapsible.Trigger
            class="w-8 h-8 flex items-center justify-center rounded-md"
            aria-label="Toggle children"
          >
            <ChevronsUpDown class="h-4 w-4" />
          </Collapsible.Trigger>
        {:else}
          <div class="w-8 h-8"></div>
        {/if}
      </div>

      <Collapsible.Content class="pl-6 space-y-1">
        {#each root.children as child (deviceDomId(child.url, child.route))}
          {@const cid = deviceDomId(child.url, child.route)}
          <div class="flex items-center space-x-2 py-0.5">
            <Checkbox
              id={cid}
              checked={deviceState.childrenSelections.get(root.url)?.has(child.route) ?? false}
              onCheckedChange={(v) => deviceState.toggleChildSelection(root.url, child.route, !!v)}
            />
            <Label for={cid} class="cursor-pointer text-sm">
              {child.route.slice(1)}: {child.meta.name}
            </Label>
            <DeviceInfoHoverCard device={child} />
          </div>
        {/each}
      </Collapsible.Content>
    </Collapsible.Root>
  {/each}
</RadioGroup.Root>
