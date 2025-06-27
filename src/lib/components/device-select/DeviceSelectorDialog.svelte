<script lang="ts">
	import { deviceService } from '$lib/stores/device.store.svelte'; // Our rune-based module
    import { Button } from '$lib/components/ui/button';
    import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { uiStore } from '$lib/stores/ui.store.svelte';
    import { LoaderCircleIcon } from '@lucide/svelte/icons';
    import DeviceList from '$lib/components/device-select/DeviceList.svelte';

	
	let isSearching = $state(true);

	$effect(() => {
		deviceService.devices()
		   
		isSearching = true;
		const t = setTimeout(() => (isSearching = false), 2000);
		return () => clearTimeout(t);
	});


	let selection = $state({
        parent: '',
        children: new Set<string>()
    });

	function confirm(event: SubmitEvent) {
		event.preventDefault();
		deviceService.selection = {
			parent: selection.parent,
			children: Array.from(selection.children)
		};
	}
	
</script>
<AlertDialog.Root open={uiStore.is('discovery')}>
	<AlertDialog.Portal>
		<AlertDialog.Overlay />
			<AlertDialog.Content
				class="fixed left-1/2 top-1/2 z-50 w-[32rem] max-w-[95vw]
					-translate-x-1/2 -translate-y-1/2 rounded-lg
					bg-card p-6 shadow-lg"
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
					{#if isSearching}
						<div class="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
							<LoaderCircleIcon class="size-6 animate-spin" />
							<span>Scanning for devices...</span>
						</div>
					{:else if deviceService.devices().length === 0}
						<div class="flex h-full flex-col items-center justify-center text-muted-foreground">
							<p>No Twinleaf devices detected.</p>
							<p>The app will continue scanning for changes.</p>
						</div>
					{:else}
						<form onsubmit={confirm}>
							<DeviceList
								devices={deviceService.deviceTree()}
								bind:selection={selection}
							/>
						</form>
					{/if}
				</div>

				<AlertDialog.Footer>
					<Button variant="outline" class="w-full" type="submit">
						Confirm
					</Button>
				</AlertDialog.Footer>
			</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>
