<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import DeviceListItem from '$lib/components/DeviceListItem.svelte';
	import { RadioGroup } from '$lib/components/ui/radio-group';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { RefreshCwIcon, LoaderCircleIcon } from '@lucide/svelte';

	import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';
	import type { PortInfo } from '$lib/bindings/PortInfo';

	// --- PROPS ---
	let { onStart = (ports: PortInfo[]) => {} } = $props();

	// --- STATE  ---
	let isLoading = $state(true);
	let devices = $state<FeDeviceMeta[]>([]);
	let error = $state<string | null>(null);
	let selectedDeviceUrl = $state<string | undefined>(undefined);
	let selectionsByParent = $state(new Map<string, Set<string>>());

	const selectedDevice = $derived(devices.find((d) => d.url === selectedDeviceUrl));

	// --- LOGIC  ---
	function getValueForRadio() {
        return selectedDeviceUrl ?? '';
    }

    function setValueFromRadio(newValue: string | null) {
        selectedDeviceUrl = (newValue === null || newValue === '') ? undefined : newValue;
    }

	async function discover() {
		isLoading = true;
		error = null;
		selectedDeviceUrl = undefined;
		selectionsByParent.clear();
		try {
			const result = await invoke<FeDeviceMeta[]>('discover_devices');
			// Sort parent devices alphabetically by name
			devices = result.sort((a, b) => a.name.localeCompare(b.name));
			if (devices.length === 1) {
				selectedDeviceUrl = devices[0].url;
			}
		} catch (e) {
			console.error('Failed to discover devices:', e);
			error = typeof e === 'string' ? e : 'An unknown error occurred.';
		} finally {
			isLoading = false;
		}
	}

	$effect(() => {
		if (!selectedDeviceUrl) return;

		if (!selectionsByParent.has(selectedDeviceUrl)) {
			const device = devices.find((d) => d.url === selectedDeviceUrl);
			const newInitialRoutes = new Set<string>();

			if (device) {
				newInitialRoutes.add(device.route);
				device.children?.forEach((child) => newInitialRoutes.add(child.route));
			}

			const newMap = new Map(selectionsByParent);
			newMap.set(selectedDeviceUrl, newInitialRoutes);
			selectionsByParent = newMap;
		}
	});

	function handleStartStreaming() {
		if (!selectedDeviceUrl) return;

		const selectionSet = selectionsByParent.get(selectedDeviceUrl) ?? new Set();

		const portsToStream: PortInfo[] = Array.from(selectionSet).map(
			(route: string) => ({
				id: route,
				url: selectedDeviceUrl!
			})
		);

		onStart(portsToStream);
	}


	onMount(discover);

	// --- DERIVED STATE  ---
	const isAnythingSelected = $derived(
		!!selectedDeviceUrl && (selectionsByParent.get(selectedDeviceUrl)?.size ?? 0) > 0
	);</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
	<div
		class="z-50 flex w-full max-w-md flex-col gap-4 rounded-2xl border bg-card p-6 text-card-foreground shadow-lg"
	>
		<div class="flex flex-col items-center text-center">
			<img src="/Twinleaf-Logo-Black.svg" alt="Twinleaf Logo" class="mb-4 h-12" />
			<div class="relative w-full">
				<h2 class="text-lg font-semibold text-zinc-900">Choose a device</h2>
				<Button
					variant="ghost"
					size="icon"
					class="absolute right-0 top-1/2 -translate-y-1/2"
					onclick={discover}
					disabled={isLoading}
				>
					<RefreshCwIcon class="size-4" />
				</Button>
			</div>
			<p class="text-sm text-zinc-500">Please select a unique serial device to capture data from.</p>
		</div>

		<div class="min-h-48">
			{#if isLoading}
				<div class="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
					<LoaderCircleIcon class="size-6 animate-spin" />
					<span>Scanning for devices...</span>
				</div>
			{:else if error}
				<div class="flex h-full flex-col items-center justify-center gap-2 text-destructive">
					<p>Error during discovery:</p>
        			<p class="text-sm whitespace-pre-line text-center">{error}</p>
				</div>
			{:else if devices.length === 0}
				<div class="flex h-full flex-col items-center justify-center text-muted-foreground">
					<p>No Twinleaf devices detected.</p>
					<p class="text-sm">Ensure they are connected and try refreshing.</p>
				</div>
			{:else}
				<RadioGroup bind:value={getValueForRadio, setValueFromRadio} class="flex flex-col gap-1.5">
					{#each devices as device (device.url)}
						<DeviceListItem
						{device}
						isSelected={selectedDeviceUrl === device.url}
						currentSelections={selectionsByParent.get(device.url) ?? new Set()}
						onSelectionChange={(route, isSelected) => {
							const selectionSet = selectionsByParent.get(device.url);
							if (!selectionSet) return;

							isSelected ? selectionSet.add(route) : selectionSet.delete(route);
							
							selectionsByParent.set(device.url, selectionSet);
						}}
					/>
					{/each}
				</RadioGroup>
			{/if}
		</div>

		<div class="flex items-center pt-0">
			<Button
				class="w-full bg-primary text-primary-foreground hover:bg-primary/90"
				onclick={handleStartStreaming}
				disabled={!isAnythingSelected || isLoading}
				autofocus
			>
				Confirm Selection
			</Button>
		</div>
	</div>
</div>