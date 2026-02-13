<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { deviceState } from '$lib/states/deviceState.svelte';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	import * as Accordion from '$lib/components/ui/accordion/index.js';
	import DataTable from './data-table/DataTable.svelte';
	import { columns } from './data-table/column';
	import { capsForDevice } from '$lib/utils';

	let selectedDevices: UiDevice[] = $derived(
		deviceState.selectedDevices.filter(d => capsForDevice(d).showInRpcPanel)
	)

	let accordionContainerEl: HTMLDivElement | undefined = $state(undefined);
	let contentHeight = $state(0);

	let containerWidth = $state(0);
	let isSmall = $derived(containerWidth < 460);

	const calculateHeight = () => {
		if (!accordionContainerEl) return;
		const containerHeight = accordionContainerEl.clientHeight;
		const triggers = accordionContainerEl.querySelectorAll<HTMLElement>('[data-accordion-trigger]');
		const items = accordionContainerEl.querySelectorAll<HTMLElement>('[data-slot="accordion-item"]');
		let totalTriggersHeight = 0;
		let totalItemBorderHeight = 0;
		triggers.forEach((trigger) => {
			totalTriggersHeight += trigger.offsetHeight;
		});
		items.forEach((item) => {
			const styles = getComputedStyle(item);
			totalItemBorderHeight += Number.parseFloat(styles.borderTopWidth);
			totalItemBorderHeight += Number.parseFloat(styles.borderBottomWidth);
		});
		if (containerHeight === 0 || (triggers.length > 0 && totalTriggersHeight === 0)) {
			return;
		}
		contentHeight = Math.max(0, containerHeight - totalTriggersHeight - totalItemBorderHeight);
	};

	$effect(() => {
		selectedDevices.map((device) => device.url + device.route).join('|');
		void tick().then(() => {
			calculateHeight();
		});
	});

	onMount(() => {
		if (!accordionContainerEl) return;

		const resizeObserver = new ResizeObserver((entries) => {
			const entry = entries[0];
			if (entry) {
				containerWidth = entry.contentRect.width;
			}
			calculateHeight();
		});
		resizeObserver.observe(accordionContainerEl);

		const initialize = async () => {
			await tick();
			containerWidth = accordionContainerEl!.clientWidth;
			calculateHeight();
		};
		initialize();

		return () => {
			resizeObserver.disconnect();
		};
	});
</script>

<div class="w-full h-full flex flex-col rounded-lg border bg-card text-card-foreground p-4">
	<div class="flex-1 min-h-0 min-w-0 overflow-hidden" bind:this={accordionContainerEl}>
		{#if selectedDevices.length}
			<Accordion.Root
				class="w-full h-full overflow-hidden"
				type="single"
				style={`--radix-accordion-content-height: ${contentHeight}px`}
			>
				{#each selectedDevices as device (device.url + device.route)}
					<Accordion.Item value={device.url + device.route}>
						<Accordion.Trigger
							class="flex w-full flex-1 select-none items-center justify-between py-4 px-2 text-sm font-medium transition-all hover:underline [&[data-state=open]>svg]:rotate-180"
						>
							<div class="text-left">
								<p class="font-semibold">{device.meta.name}</p>
								<p class="text-xs text-muted-foreground">Route: {device.route}</p>
							</div>
						</Accordion.Trigger>

						<Accordion.Content class="p-0" forceMount>
							<div class="h-[var(--radix-accordion-content-height)] box-border p-2">
								<DataTable {columns} data={device.rpcs} {device} {isSmall} />
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