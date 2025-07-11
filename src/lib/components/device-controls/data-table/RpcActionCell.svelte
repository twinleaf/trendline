<script lang="ts">
	import type { RpcMeta } from '$lib/bindings/RpcMeta';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { invoke } from '@tauri-apps/api/core';
    import type { RpcError } from '$lib/bindings/RpcError';

	let { rpc, device }: { rpc: RpcMeta, device: UiDevice } = $props();

	let inputValue = $state('');
	let isLoading = $state(false);

	async function executeRpc(args: any) {
		isLoading = true;
		try {
			const result = await invoke<RpcError | null>('execute_rpc', {
				portUrl: device.url,
				deviceRoute: device.route,
				name: rpc.name,
				args: args
			});
			console.log(`RPC ${rpc.name} successful, result:`, result);
			if (rpc.writable) {
				inputValue = ''; // Clear input on success
			}
		} catch (e) {
			console.error(`Failed to execute RPC ${rpc.name}:`, e);
			// You could show an error toast here
		} finally {
			isLoading = false;
		}
	}

	function handleSet() {
		if (inputValue === undefined || inputValue === null || inputValue === '') return;
		let arg: any = inputValue;
		if (rpc.arg_type.includes('f') || rpc.arg_type.includes('u') || rpc.arg_type.includes('i')) {
			arg = Number(inputValue);
			if (isNaN(arg)) {
				console.error("Invalid number for RPC", rpc.name);
				return;
			}
		}
		executeRpc(arg);
	}

	function handleAction() {
		executeRpc(null); // Actions have no arguments
	}
</script>

{#if !rpc.readable && !rpc.writable}
	<!-- Case 1: Action-only RPC (e.g., a command like 'reboot') -->
	<Button
		size="sm"
		variant="outline"
		class="h-8"
		onclick={handleAction}
		disabled={isLoading}
	>
		{isLoading ? '...' : 'Execute'}
	</Button>

{:else if rpc.writable}
	<!-- Case 2: Writable RPC -->
	<div class="flex w-full min-w-0 items-center gap-2">
		<Input
			type={rpc.arg_type.includes('f') || rpc.arg_type.includes('i') || rpc.arg_type.includes('u') ? 'number' : 'text'}
			class="h-8 flex-1 bg-transparent font-mono text-xs"
			placeholder={`Value: ${JSON.stringify(rpc.value) ?? 'N/A'}`}
			bind:value={inputValue}
			onkeydown={(e) => e.key === 'Enter' && handleSet()}
			disabled={isLoading}
		/>
		<Button
			size="sm"
			variant="outline"
			class="h-8 w-16 rpc-set-button"
			onclick={handleSet}
			disabled={isLoading}
		>
			{isLoading ? '...' : 'Set'}
		</Button>
	</div>

{:else}
	<!-- Case 3: Read-only RPC -->
	<div class="flex h-8 min-w-0 items-center rounded-md border border-input bg-transparent px-3 py-2 text-xs ring-offset-background">
		<span class="font-mono text-muted-foreground truncate" title={JSON.stringify(rpc.value)}>
			{JSON.stringify(rpc.value) ?? 'N/A'}
		</span>
	</div>
{/if}
