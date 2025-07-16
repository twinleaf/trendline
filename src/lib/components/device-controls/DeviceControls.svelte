<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	import * as Accordion from '$lib/components/ui/accordion/index.js';
	import DataTable from './data-table/DataTable.svelte';
	import { columns } from './data-table/column';

	let selectedDevices: UiDevice[] = $derived(deviceState.selectedDevices);

	// This logic remains the same
	let accordionContainerEl: HTMLDivElement | undefined = $state(undefined);
	let contentHeight = $state(0);

	$effect(() => {
		if (!accordionContainerEl) return;
		
		const observer = new ResizeObserver(() => {
			if (!accordionContainerEl) return;
			const containerHeight = accordionContainerEl.clientHeight;
			const triggers = accordionContainerEl.querySelectorAll<HTMLElement>(
				'[data-accordion-trigger]'
			);
			if (triggers.length === 0) return;

			let totalTriggersHeight = 0;
			triggers.forEach((trigger) => {
				totalTriggersHeight += trigger.offsetHeight;
			});
			
			contentHeight = Math.max(0, containerHeight - totalTriggersHeight);
		});

		observer.observe(accordionContainerEl);
		return () => observer.disconnect();
	});
</script>

<div class="w-full h-full flex flex-col space-y-4 rounded-lg border bg-card text-card-foreground p-4">
	<h3 class="text-lg font-semibold">Device Controls</h3>

	<div class="flex-1 min-h-0" bind:this={accordionContainerEl}>
		{#if selectedDevices.length}
			<Accordion.Root
				class="w-full h-full"
				type="single"
				style={`--radix-accordion-content-height: ${contentHeight}px`}
			>
				{#each selectedDevices as device (device.url + device.route)}
					<Accordion.Item value={device.url + device.route}>
						<Accordion.Trigger
							class="flex w-full flex-1 select-none items-center justify-between py-4 text-sm font-medium transition-all hover:underline [&[data-state=open]>svg]:rotate-180"
						>
							<div class="text-left">
								<p class="font-semibold">{device.meta.name}</p>
								<p class="text-xs text-muted-foreground">Route: {device.route}</p>
							</div>
						</Accordion.Trigger>

						<Accordion.Content
							class="overflow-hidden data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down"
						>
							<div class="h-[var(--radix-accordion-content-height)] overflow-y-auto">
								<div class="p-2">
									<DataTable {columns} data={device.rpcs} {device} />
								</div>
							</div>
						</Accordion.Content>
					</Accordion.Item>
				{/each}
			</Accordion.Root>
		{:else}
			<p class="text-sm text-muted-foreground">No streams or devices selected.</p>
		{/if}
	</div>
</div>