<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { uiState } from '$lib/states/uiState.svelte';
	import { LoaderCircleIcon, PlusIcon } from '@lucide/svelte/icons';
	import DeviceList from '$lib/components/device-select/DeviceList.svelte';
	import { invoke } from '@tauri-apps/api/core';

	let devices = $derived(deviceState.deviceTree);
	let selectedParent = $state('');
	let manualUrl = $state('tcp://localhost');

	// We need this state variable to control the manual connect dialog
	let manualConnectDialogOpen = $state(false);

	let isConfirmDisabled = $derived.by(() => {
		if (deviceState.deviceTree.length === 0) {
			return true;
		}
		const portInfo = deviceState.getPort(selectedParent);
		if (!portInfo) {
			return true;
		}
		return portInfo.state !== 'Streaming';
	});

	$effect(() => {
		if (devices.length > 0 && !selectedParent) {
			selectedParent = devices[0].url;
		}
	});

	// Simplified function: fire-and-forget without toast notifications.
	function handleManualConnect() {
		if (!manualUrl) return;
		invoke('connect_to_port', { portUrl: manualUrl });
		manualConnectDialogOpen = false;
	}

	function confirm(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedParent) {
			console.warn('No parent device selected.');
			return;
		}
		const selectedChildren = deviceState.childrenSelections.get(selectedParent) ?? new Set<string>();

		const payload = {
			portUrl: selectedParent,
			childrenRoutes: Array.from(selectedChildren)
		};

		deviceState.selection = payload;
		invoke('confirm_selection', payload);
		uiState.close();
		manualConnectDialogOpen = false;
	}
</script>

<AlertDialog.Root open={uiState.is('discovery')}>
	<AlertDialog.Content
		class="fixed left-1/2 top-1/2 z-50 w-[32rem] max-w-[95vw] -translate-x-1/2 -translate-y-1/2 rounded-lg bg-card p-6 shadow-lg"
	>
		<AlertDialog.Header class="flex flex-col items-center space-y-2 text-center">
			<img src="/Twinleaf-Logo-Black.svg" alt="Twinleaf Logo" class="mb-4 h-12" />
			<AlertDialog.Title class="text-lg font-semibold text-zinc-900">
				Device Status
			</AlertDialog.Title>
			<AlertDialog.Description class="text-sm text-zinc-500">
				Please select a unique serial device to stream from.
			</AlertDialog.Description>
		</AlertDialog.Header>

		<div class="min-h-48">
			{#if devices.length === 0}
				<div class="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
					<LoaderCircleIcon class="size-6 animate-spin" />
					<span>Scanning for devices...</span>
				</div>
			{:else}
				<form id="device-select-form" onsubmit={confirm}>
					<DeviceList devices={deviceState.deviceTree} bind:selectedParent={selectedParent} />
					<button type="submit" class="hidden" aria-hidden="true" tabindex="-1"></button>
				</form>
			{/if}
		</div>
		<AlertDialog.Footer>
			<div class="flex w-full justify-between">
				<Button
					variant="outline"
					class="p-2"
					aria-label="Add network device"
					onclick={() => (manualConnectDialogOpen = true)}
				>
					<PlusIcon class="size-4" />
				</Button>

				<AlertDialog.Action type="submit" form="device-select-form" disabled={isConfirmDisabled}>
					Confirm
				</AlertDialog.Action>
			</div>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>

<Dialog.Root open={manualConnectDialogOpen}>
	<Dialog.Overlay />
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Connect to Network Device</Dialog.Title>
			<Dialog.Description>Enter the URL of the device you want to connect to.</Dialog.Description>
		</Dialog.Header>
		<form onsubmit={handleManualConnect} class="py-4">
			<Input
				id="manual-url"
				placeholder="e.g., tcp://localhost"
				bind:value={manualUrl}
				class="font-mono"
			/>

			<Dialog.Footer class="mt-4">
				<Dialog.Close>
					{#snippet child({ props })}
						<Button {...props} type="submit">Connect</Button>
					{/snippet}
				</Dialog.Close>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>