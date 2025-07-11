<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	import * as Accordion from '$lib/components/ui/accordion/index.js';

	// Import the data table and its columns
	import DataTable from './data-table/DataTable.svelte';
	import { columns } from './data-table/column';

	let selectedDevices: UiDevice[] = $derived(deviceState.selectedDevices);
</script>

<div class="w-full space-y-4 rounded-lg border bg-card text-card-foreground p-4">
	<h3 class="text-lg font-semibold">Device Controls</h3>
	    <div class="flex-1 overflow-y-auto">
			{#if selectedDevices.length}
				<Accordion.Root class="w-full" type="multiple">
					{#each selectedDevices as device (device.url + device.route)}
						<Accordion.Item value={device.url + device.route} class="border-b">
							<Accordion.Trigger class="flex w-full flex-1 select-none items-center justify-between py-4 text-sm font-medium transition-all hover:underline [&[data-state=open]>svg]:rotate-180">
								<div class="text-left">
									<p class="font-semibold">{device.meta.name}</p>
									<p class="text-xs text-muted-foreground">Route: {device.route}</p>
								</div>
							</Accordion.Trigger>
							<Accordion.Content class="overflow-hidden data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down">
								<div class="p-2">
									<DataTable
										{columns}
										data={device.rpcs}
										{device}
									/>
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