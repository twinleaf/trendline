<script lang="ts">
  import { useNodes, useEdges, Handle, Position, getIncomers, type Node, type NodeProps } from '@xyflow/svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { mathState } from '$lib/states/mathState.svelte';
  import type { DataColumnId } from '$lib/bindings/DataColumnId';
  import type { PipelineId } from '$lib/bindings/PipelineId';
  import type { MathChannelConfig } from '$lib/bindings/MathChannelConfig';
  import type { BiquadParams } from '$lib/bindings/BiquadParams';
  import type { BiquadKind } from '$lib/bindings/BiquadKind';
  import type { MathOp } from '$lib/bindings/MathOp';
  import InfoIcon from "@lucide/svelte/icons/info";
  import * as Select from "$lib/components/ui/select/index.js";
  import * as HoverCard from "$lib/components/ui/hover-card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";

  type TData = {
    ttype?: 'Arithmetic'|'Biquad';
    op?: MathOp;
    kind?: BiquadKind;
    cutoff_hz?: number;
    q?: number;
    outKey?: DataColumnId;
    pid?: PipelineId;
  };

  let { id, data }: NodeProps<Node<TData>> = $props();

  const nodes = useNodes();
  const edges = useEdges();

  function setData(patch: Record<string, unknown>) {
    nodes.update(ns => ns.map(n => n.id === id ? { ...n, data: { ...n.data, ...patch } } : n));
  }

  function resolveUpstreamKeys(): DataColumnId[] {
    const me = nodes.current.find(n => n.id === id);
    if (!me) return [];
    const incomers = getIncomers(me, nodes.current, edges.current);
    const keys: DataColumnId[] = [];
    for (const node of incomers) {
      const k = (node.data && (node.data.key || node.data.outKey)) as DataColumnId | undefined;
      if (k) keys.push(k);
    }
    return keys;
  }

  async function build() {
    const keys = resolveUpstreamKeys();
    let spec: MathChannelConfig | null = null;

    if (data.ttype === 'Arithmetic') {
      if (!data.op) { alert('Pick an arithmetic op.'); return; }
      if (keys.length < 2) { alert('Arithmetic needs 2 inputs. Connect two upstream nodes.'); return; }
      spec = { Arithmetic: { a: keys[0], op: data.op, b: keys[1] } };
    } else if (data.ttype === 'Biquad') {
      if (keys.length < 1) { alert('Biquad needs 1 input.'); return; }
      const params: BiquadParams = { cutoff_hz: Number(data.cutoff_hz ?? 10), q: Number(data.q ?? 0.707) };
      if (!data.kind) { alert('Pick a biquad kind.'); return; }
      spec = { Biquad: { src: keys[0], kind: data.kind, params } };
    } else {
      alert('Choose a transform type.');
      return;
    }

    try {
      const [pid, outKey] = await invoke<[PipelineId, DataColumnId]>('create_math_channel', { spec });
      setData({ pid, outKey });
      mathState.register({ key: outKey, pid, spec });
    } catch (e) {
      console.error('create_math_channel failed:', e);
      alert('Failed to build math channel.');
    }
  }

  const transformTypes = [
    { value: "", label: "Select type..." },
    { value: "Arithmetic", label: "Arithmetic" },
    { value: "Biquad", label: "Biquad" }
  ];

  const arithmeticOps = [
    { value: "", label: "Select op..." },
    { value: "Add", label: "Add (+)" },
    { value: "Sub", label: "Subtract (-)" },
    { value: "Mul", label: "Multiply (×)" },
    { value: "Div", label: "Divide (÷)" }
  ];

  const biquadKinds = [
    { value: "", label: "Select kind..." },
    { value: "Lowpass", label: "Lowpass" },
    { value: "Highpass", label: "Highpass" },
    { value: "Bandpass", label: "Bandpass" }
  ];

  let ttypeValue = $state(data.ttype || "");
  let opValue = $state(data.op || "");
  let kindValue = $state(data.kind || "");

  const ttypeContent = $derived(
    transformTypes.find((t) => t.value === ttypeValue)?.label ?? "Select type..."
  );

  const opContent = $derived(
    arithmeticOps.find((o) => o.value === opValue)?.label ?? "Select op..."
  );

  const kindContent = $derived(
    biquadKinds.find((k) => k.value === kindValue)?.label ?? "Select kind..."
  );

  // Sync with data prop changes
  $effect(() => {
    if (data.ttype !== ttypeValue) ttypeValue = data.ttype || "";
    if (data.op !== opValue) opValue = data.op || "";
    if (data.kind !== kindValue) kindValue = data.kind || "";
  });
</script>

<div class="rounded-md border bg-card text-card-foreground shadow p-3 min-w-[280px]">
  <div class="flex justify-around items-center mb-3 px-8">
    <div class="flex flex-col items-center">
      <Handle 
        type="target" 
        position={Position.Top} 
        id="a"
      />
      <div class="text-[10px] text-muted-foreground mt-1 text-center">A</div>
    </div>
    <div class="flex flex-col items-center">
      <Handle 
        type="target" 
        position={Position.Top} 
        id="b"
      />
      <div class="text-[10px] text-muted-foreground mt-1 text-center">B</div>
    </div>
  </div>

  <div class="pt-3 border-t space-y-2">
    <div class="flex items-center justify-between mb-2">
      <div class="text-xs font-semibold">Transform</div>
      {#if data.outKey}
        <HoverCard.Root>
          <HoverCard.Trigger>
            <InfoIcon class="h-3 w-3 opacity-60 hover:opacity-100 transition-opacity" />
          </HoverCard.Trigger>
          <HoverCard.Content class="w-80">
            <div class="space-y-1">
              <h4 class="text-xs font-semibold">Output Key</h4>
              <pre class="text-[10px] bg-muted p-2 rounded overflow-auto">{JSON.stringify(data.outKey, null, 2)}</pre>
            </div>
          </HoverCard.Content>
        </HoverCard.Root>
      {/if}
    </div>

    <div class="nodrag" onpointerdown={(e) => e.stopPropagation()}>
      <Select.Root
        type="single"
        name="transformType"
        bind:value={ttypeValue}
        onValueChange={(v) => setData({ ttype: v as any })}
      >
        <Select.Trigger class="w-full h-8 text-xs">
          {ttypeContent}
        </Select.Trigger>
        <Select.Content>
          <Select.Group>
            {#each transformTypes as type (type.value)}
              <Select.Item
                value={type.value}
                label={type.label}
                disabled={type.value === ""}
                class="text-xs"
              >
                {type.label}
              </Select.Item>
            {/each}
          </Select.Group>
        </Select.Content>
      </Select.Root>
    </div>

    {#if data.ttype === 'Arithmetic'}
      <div class="nodrag" onpointerdown={(e) => e.stopPropagation()}>
        <Select.Root
          type="single"
          name="arithmeticOp"
          bind:value={opValue}
          onValueChange={(v) => setData({ op: v as any })}
        >
          <Select.Trigger class="w-full h-8 text-xs">
            {opContent}
          </Select.Trigger>
          <Select.Content>
            <Select.Group>
              {#each arithmeticOps as op (op.value)}
                <Select.Item
                  value={op.value}
                  label={op.label}
                  disabled={op.value === ""}
                  class="text-xs"
                >
                  {op.label}
                </Select.Item>
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>
      </div>
    {/if}

    {#if data.ttype === 'Biquad'}
      <div class="space-y-2">
        <div class="nodrag" onpointerdown={(e) => e.stopPropagation()}>
          <Select.Root
            type="single"
            name="biquadKind"
            bind:value={kindValue}
            onValueChange={(v) => setData({ kind: v as any })}
          >
            <Select.Trigger class="w-full h-8 text-xs">
              {kindContent}
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                {#each biquadKinds as kind (kind.value)}
                  <Select.Item
                    value={kind.value}
                    label={kind.label}
                    disabled={kind.value === ""}
                    class="text-xs"
                  >
                    {kind.label}
                  </Select.Item>
                {/each}
              </Select.Group>
            </Select.Content>
          </Select.Root>
        </div>

        <div class="flex gap-2 nodrag" onpointerdown={(e) => e.stopPropagation()}>
          <Input
            type="number"
            min="0.001"
            step="0.1"
            placeholder="Cutoff Hz"
            class="text-xs h-8"
            value={data.cutoff_hz ?? 10}
            oninput={(e) => setData({ cutoff_hz: Number((e.target as HTMLInputElement).value) })}
          />
          <Input
            type="number"
            min="0.01"
            step="0.01"
            placeholder="Q"
            class="text-xs h-8 w-20"
            value={data.q ?? 0.707}
            oninput={(e) => setData({ q: Number((e.target as HTMLInputElement).value) })}
          />
        </div>
      </div>
    {/if}

    <div class="nodrag" onpointerdown={(e) => e.stopPropagation()}>
      <Button
        variant="default"
        size="sm"
        class="w-full text-xs h-8"
        onclick={build}
      >
        Build Transform
      </Button>
    </div>

    <div class="text-[10px] text-muted-foreground text-center">
      {#if data.outKey}
        <span class="text-green-600">✓ Built</span>
      {:else}
        <span>Not built yet</span>
      {/if}
    </div>
  </div>

  <div class="mt-3 pt-3 border-t">
    <div class="text-[10px] text-muted-foreground text-center mb-1">Output</div>
      <Handle type="source" position={Position.Bottom} id="out" />
  </div>
</div>