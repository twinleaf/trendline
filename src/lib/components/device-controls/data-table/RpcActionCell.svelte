<script lang="ts">
	import type { RpcMeta } from '$lib/bindings/RpcMeta';
	import type { UiDevice } from '$lib/bindings/UiDevice';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { invoke } from '@tauri-apps/api/core';
	import type { RpcError } from '$lib/bindings/RpcError';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { TriangleAlert } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { onMount, onDestroy } from 'svelte';

	let { rpc, device }: { rpc: RpcMeta; device: UiDevice } = $props();

	let inputValue = $state('');
	let isLoading = $state(false);
	let rpcError = $state<RpcError | null>(null);
	let displayedError = $state<RpcError | null>(null);
	let isPopoverOpen = $state(false);
	let observer: IntersectionObserver;

	let inputContainerEl = $state<HTMLDivElement | undefined>();
	let executeButtonContainerEl = $state<HTMLDivElement | undefined>();

	onMount(() => {
		observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[0];
				if (!entry.isIntersecting && isPopoverOpen) {
					rpcError = null;
				}
			},
			{ threshold: 0 }
		);
	});

	onDestroy(() => {
		if (observer) {
			observer.disconnect();
		}
	});

	$effect(() => {
		const elementToObserve = rpc.writable ? inputContainerEl : executeButtonContainerEl;

		if (observer && elementToObserve) {
			observer.disconnect();
			observer.observe(elementToObserve);
		}
	});

	$effect(() => {
		isPopoverOpen = !!rpcError;
	});

	$effect(() => {
		if (rpcError) {
			displayedError = rpcError;
		}
	});

	$effect(() => {
		if (rpcError) {
			const timerId = setTimeout(() => {
				rpcError = null;
			}, 3000);
			return () => clearTimeout(timerId);
		}
	});

	$effect(() => {
		if (!isPopoverOpen) {
			untrack(() => {
				if (rpcError) rpcError = null;
			});
		}
	});

	$effect(() => {
		if (rpcError && inputContainerEl) {
			inputContainerEl.querySelector('input')?.focus();
		}
	});

	function formatRpcError(err: RpcError | null): string {
		if (!err) return '';
		switch (err.type) {
			case 'ExecError':
				const code = err.payload.error;
				let formattedCode: string;

				if (typeof code === 'string') {
					formattedCode = code.replace(/([A-Z])/g, ' $1').trim();
				} else {
					formattedCode = `Unknown (${code.Unknown})`;
				}
				return `Execution Error: ${formattedCode}`;

			case 'SendFailed':
				return `Send Failed: ${err.payload}`;
			case 'RecvFailed':
				return `Receive Failed: ${err.payload}`;
			case 'AppLogic':
				return err.payload;
			case 'TypeError':
				return 'Type Error: The provided value has the wrong type.';
			default:
				return 'An unknown error occurred.';
		}
	}

	async function executeRpc(args: any) {
		isLoading = true;
		rpcError = null;
		try {
			await invoke('execute_rpc', {
				portUrl: device.url,
				deviceRoute: device.route,
				name: rpc.name,
				args: args
			});

			console.log(`RPC ${rpc.name} successful.`);
			if (rpc.writable) {
				inputValue = '';
			}
		} catch (err) {
			console.error(`Failed to execute RPC ${rpc.name}:`, err);
			rpcError = err as RpcError;
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
				rpcError = { type: 'AppLogic', payload: 'Invalid number format.' };
				return;
			}
		}
		executeRpc(arg);
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
			event.preventDefault();
			handleSet();
		}
	}

	function handleAction() {
		executeRpc(null);
	}
</script>

{#if !rpc.readable && !rpc.writable}
	<Popover.Root bind:open={isPopoverOpen}>
		<div class="flex justify-end" bind:this={executeButtonContainerEl}>
			<Button
				variant="default"
				size="sm"
				class="h-8 w-20 justify-center"
				onclick={handleAction}
				disabled={isLoading}
			>
				{isLoading ? '...' : 'Execute'}
			</Button>
		</div>
		<Popover.Content
			customAnchor={executeButtonContainerEl}
			side="top"
			sideOffset={4}
			class="z-10 w-auto rounded-md bg-destructive p-2 text-sm text-destructive-foreground"
			onOpenAutoFocus={(e) => e.preventDefault()}
		>
			<div class="flex items-center gap-2">
				<TriangleAlert class="size-4" />
				<p>{formatRpcError(displayedError)}</p>
			</div>
		</Popover.Content>
	</Popover.Root>
{:else if rpc.writable}
	<Popover.Root bind:open={isPopoverOpen}>
		<div class="flex w-full min-w-0 items-center gap-2">
			<div bind:this={inputContainerEl} class="flex-1">
				<Input
					type={
						rpc.arg_type.includes('f') || rpc.arg_type.includes('i') || rpc.arg_type.includes('u')
							? 'number'
							: 'text'
					}
					class="h-8 w-full bg-transparent font-mono text-xs"
					placeholder={`${JSON.stringify(rpc.value) ?? 'N/A'}`}
					bind:value={inputValue}
					onkeydown={handleKeyDown}
					oninput={() => (rpcError = null)}
					onblur={() => (rpcError = null)}
					aria-invalid={!!rpcError}
					disabled={isLoading}
					autocomplete="off"
				/>
			</div>
			<Button
				variant="default"
				size="sm"
				class="h-8 w-20 justify-center"
				onclick={handleSet}
				disabled={isLoading}
			>
				{isLoading ? '...' : 'Set'}
			</Button>
		</div>
		<Popover.Content
			customAnchor={inputContainerEl}
			side="top"
			sideOffset={4}
			class="z-10 w-auto rounded-md bg-destructive p-2 text-sm text-destructive-foreground"
			onOpenAutoFocus={(e) => e.preventDefault()}
		>
			<div class="flex items-center gap-2">
				<TriangleAlert class="size-4" />
				<p>{formatRpcError(displayedError)}</p>
			</div>
		</Popover.Content>
	</Popover.Root>
{:else}
	<div
		class="flex h-8 min-w-0 items-center rounded-md border border-input bg-transparent px-3 py-2 text-xs ring-offset-background"
	>
		<span class="font-mono text-muted-foreground truncate" title={JSON.stringify(rpc.value)}>
			{JSON.stringify(rpc.value) ?? 'N/A'}
		</span>
	</div>
{/if}