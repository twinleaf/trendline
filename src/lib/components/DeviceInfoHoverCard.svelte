<script lang="ts">
	import * as HoverCard from '$lib/components/ui/hover-card';
	import { InfoIcon } from '@lucide/svelte';
	import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';

	let { device }: { device: FeDeviceMeta } = $props();
</script>

<HoverCard.Root openDelay={200} closeDelay={100}>
	<HoverCard.Trigger href={null} class="cursor-default">
		<InfoIcon class="size-4 text-zinc-400" />
	</HoverCard.Trigger>
	<HoverCard.Content class="w-auto" side="top">
		<div class="flex flex-col space-y-1 text-sm">
			<div class="flex justify-between">
				<span class="font-semibold text-zinc-700">Serial #:</span>
				<span class="ml-4 font-mono text-zinc-500">{device.serial}</span>
			</div>
			<div class="flex justify-between">
				<span class="font-semibold text-zinc-700">Firmware:</span>
				<span class="ml-4 font-mono text-zinc-500">{device.firmware ?? 'N/A'}</span>
			</div>
                <div class="flex justify-between">
				<span class="font-semibold text-zinc-700">Route:</span>
				<span class="ml-4 font-mono text-zinc-500">{device.url ?? 'N/A'}</span>
			</div>
		</div>
	</HoverCard.Content>
</HoverCard.Root>