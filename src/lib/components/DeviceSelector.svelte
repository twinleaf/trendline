<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import DeviceListItem from '$lib/components/DeviceListItem.svelte';
	import { RadioGroup } from '$lib/components/ui/radio-group';
	import { RefreshCwIcon, LoaderCircleIcon } from '@lucide/svelte/icons';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';

	import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';
	import type { PortInfo } from '$lib/bindings/PortInfo';

	// --- PROPS ---
	let { onConfirm = (ports: PortInfo[], devices: FeDeviceMeta[]) => {} } = $props();

	// --- STATE  ---
	let isLoading = $state(true);
	let devices = $state<FeDeviceMeta[]>([]);
	let error = $state<string | null>(null);
	let selectedDeviceUrl = $state<string | undefined>(undefined);
	let selectionsByParent = $state(new Map<string, Set<string>>());

	// --- DERIVED STATE  ---
	const isAnythingSelected = $derived(
		!!selectedDeviceUrl && (selectionsByParent.get(selectedDeviceUrl)?.size ?? 0) > 0
	);

	// --- LOGIC  ---
	function getValueForRadio() {
		return selectedDeviceUrl ?? '';
	}

	function setValueFromRadio(newValue: string | null) {
		selectedDeviceUrl = newValue === null || newValue === '' ? undefined : newValue;
	}

	async function discover() {
		isLoading = true;
		error = null;
		selectedDeviceUrl = undefined;
		selectionsByParent.clear();
		try {
			const result = await invoke<FeDeviceMeta[]>('discover_devices');
			devices = result.sort((a, b) => a.name.localeCompare(b.name));
			if (devices.length > 0) {
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

	$effect(() => {
		if (!isLoading && selectedDeviceUrl) {
			queueMicrotask(() => {
				const elementToFocus = document.getElementById(selectedDeviceUrl!);
				elementToFocus?.focus();
			});
		}
	});

	function handleConfirm() {
		if (!selectedDeviceUrl) return;
		const selectionSet = selectionsByParent.get(selectedDeviceUrl) ?? new Set();
		const portsToStream: PortInfo[] = Array.from(selectionSet).map((route: string) => ({
			id: route,
			url: selectedDeviceUrl!
		}));
		onConfirm(portsToStream, devices);
	}

	onMount(discover);
</script>

<AlertDialog.Header class="flex flex-col items-center space-y-2 text-center">
	<img src="/Twinleaf-Logo-Black.svg" alt="Twinleaf Logo" class="mb-4 h-12" />

	<AlertDialog.Title class="text-lg font-semibold text-zinc-900">
		Choose a device
	</AlertDialog.Title>

	<AlertDialog.Description class="text-sm text-zinc-500">
		Please select a unique device to capture data from. 
	</AlertDialog.Description>

	{#if !isLoading}
		<Button variant="ghost" size="sm" onclick={discover}>
			<RefreshCwIcon class="size-4" />
			<span>Refresh</span>
		</Button>
	{/if}
</AlertDialog.Header>

<form
	onsubmit={(event) => {
		event.preventDefault(); 
		handleConfirm();
	}}
>

<div class="min-h-48">
	{#if isLoading}
		<div class="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
			<LoaderCircleIcon class="size-6 animate-spin" />
			<span>Scanning for devices...</span>
		</div>
	{:else if error}
		<div class="flex h-full flex-col items-center justify-center gap-2 text-destructive">
			<p>Error during discovery:</p>
			<p class="whitespace-pre-line text-center text-sm">{error}</p>
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

<AlertDialog.Footer>
	<AlertDialog.Action
		type="submit"
		class="w-full"
		disabled={!isAnythingSelected || isLoading}
	>
		Confirm Selection
	</AlertDialog.Action>
</AlertDialog.Footer>

</form>