<script lang="ts">
    import * as HoverCard from '$lib/components/ui/hover-card';
    import { InfoIcon } from '@lucide/svelte';
	import type { UiDevice } from '$lib/bindings/UiDevice';

    let { device }: { device: UiDevice } = $props();
</script>

<HoverCard.Root openDelay={200} closeDelay={100}>
    <HoverCard.Trigger href={null} class="cursor-default">
        <InfoIcon class="size-4 text-zinc-400" />
    </HoverCard.Trigger>
    <HoverCard.Content class="w-auto" side="top">
        <div class="flex flex-col space-y-1 text-sm">
            <div class="flex justify-between">
                <span class="font-semibold text-zinc-700">Serial:</span>
                <span class="ml-4 font-mono text-zinc-500">{device.meta.serial_number}</span>
            </div>
            <div class="flex justify-between">
                <span class="font-semibold text-zinc-700">Firmware:</span>
                <span class="ml-4 font-mono text-zinc-500">{device.meta.firmware_hash}</span>
            </div>
                <div class="flex justify-between">
                <span class="font-semibold text-zinc-700">Route:</span>
                <span class="ml-4 font-mono text-zinc-500">{device.route}</span>
            </div>
        </div>
    </HoverCard.Content>
</HoverCard.Root>