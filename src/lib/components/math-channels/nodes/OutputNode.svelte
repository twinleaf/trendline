<script lang="ts">
  import { useNodes, useEdges, Handle, Position, getIncomers, type Node, type NodeProps } from '@xyflow/svelte';
  import type { DataColumnId } from '$lib/bindings/DataColumnId';
  import InfoIcon from "@lucide/svelte/icons/info";
  import * as HoverCard from "$lib/components/ui/hover-card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";

  let { id, data }: NodeProps<Node<{ attachedKey?: DataColumnId }>> = $props();

  const nodes = useNodes();
  const edges = useEdges();

  function pickUpstream() {
    const me = nodes.current.find(n => n.id === id);
    if (!me) return;
    const incomers = getIncomers(me, nodes.current, edges.current);
    const first = incomers[0];
    const key = first?.data?.outKey || first?.data?.key;
    if (key) {
      nodes.update(ns => ns.map(n => n.id === id ? { ...n, data: { ...n.data, attachedKey: key } } : n));
    }
  }
</script>

<div class="rounded-md border bg-card text-card-foreground shadow p-3 min-w-[220px]">
    <div class="mb-3">
    <Handle type="target" position={Position.Top} />
    <div class="text-[10px] text-muted-foreground text-center">Input</div>
  </div>

  <div class="pt-3 border-t">
    <div class="flex items-center justify-between mb-2">
      <div class="text-xs font-semibold">Output</div>
      {#if data.attachedKey}
        <HoverCard.Root>
          <HoverCard.Trigger>
            <InfoIcon class="h-3 w-3 opacity-60 hover:opacity-100 transition-opacity" />
          </HoverCard.Trigger>
          <HoverCard.Content class="w-80">
            <div class="space-y-1">
              <h4 class="text-xs font-semibold">Attached Key</h4>
              <pre class="text-[10px] bg-muted p-2 rounded overflow-auto">{JSON.stringify(data.attachedKey, null, 2)}</pre>
            </div>
          </HoverCard.Content>
        </HoverCard.Root>
      {/if}
    </div>

    <Button
      variant="outline"
      size="sm"
      class="w-full text-xs h-8 nodrag"
      onclick={pickUpstream}
      onpointerdown={(e) => e.stopPropagation()}
    >
      Attach upstream
    </Button>

    <div class="mt-2 text-[10px] text-muted-foreground text-center">
      {#if data.attachedKey}
        <span class="text-green-600">✓ Attached</span>
      {:else}
        <span>Nothing attached</span>
      {/if}
    </div>
  </div>
</div>