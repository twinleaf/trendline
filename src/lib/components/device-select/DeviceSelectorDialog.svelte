<script lang="ts">
	import { deviceState } from '$lib/states/deviceState.svelte';
    import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { uiState } from '$lib/states/uiState.svelte';
    import { LoaderCircleIcon } from '@lucide/svelte/icons';
    import DeviceList from '$lib/components/device-select/DeviceList.svelte';
    import { invoke } from '@tauri-apps/api/core';

	
	let devices = $derived(deviceState.deviceTree);

	let selectedParent = $state('');
	let childrenSelections = $state(new Map<string, Set<string>>());

	$effect(() => {
		if (devices.length > 0 && !selectedParent) {
        	selectedParent = devices[0].url;
		}
	});


	$effect(() => {
		const parentUrl = selectedParent;
		if (!parentUrl || childrenSelections.has(parentUrl)) {
			return;
		}
		const parentDevice = devices.find((d) => d.route === parentUrl);
		if (parentDevice) {
        	const allChildrenRoutes = new Set(parentDevice.children.map((c) => c.route));
			const newChildrenSelections = new Map(childrenSelections);
			newChildrenSelections.set(parentUrl, allChildrenRoutes);
        	childrenSelections = newChildrenSelections;
		}
	});

	function confirm(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedParent) { 
			console.warn('No parent device selected.');
			return;
		}
		const selectedChildren = childrenSelections.get(selectedParent) ?? new Set<string>();

		const payload = {
			portUrl: selectedParent,
			childrenRoutes: Array.from(selectedChildren)
		};

		deviceState.selection = payload;
		
		invoke('confirm_selection', payload );

		uiState.close();
	}
	
</script>
<AlertDialog.Root open={uiState.is('discovery')}>
	<AlertDialog.Portal>
		<AlertDialog.Overlay />
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
					<!-- We give the form an ID so the button in the footer can reference it. -->
					<form id="device-select-form" onsubmit={confirm}>
						<DeviceList
							devices={deviceState.deviceTree}
							bind:selectedParent={selectedParent}
							bind:childrenSelections={childrenSelections}
						/>
						<button type="submit" class="hidden" aria-hidden="true" tabindex="-1"></button>
					</form>
				{/if}
			</div>

				<AlertDialog.Action
					type="submit"
					form="device-select-form"
					disabled={devices.length === 0}
				>
					Confirm
				</AlertDialog.Action>
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>