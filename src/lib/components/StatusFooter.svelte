<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import * as Breadcrumb from '$lib/components/ui/breadcrumb';
	import { CircleCheck, CircleX, LoaderCircle } from '@lucide/svelte';
    import { ioStore } from '$lib/stores/io.store.svelte';
    import { deviceStore } from '$lib/stores/device.store.svelte';

	const pathSegments = $derived(ioStore.loggingPath.split('/').filter(Boolean));
</script>
<div class="flex h-9 items-center justify-between border-t bg-card shadow-xs rounded-md border p-1 text-sm">
	<!-- Left Side: Activity / Logging Path -->
	<div class="flex items-center gap-4 flex-1 min-w-0">
        {#if ioStore.isLogging}
		<Breadcrumb.Root class="text-muted-foreground">
			<Breadcrumb.List>
				<Breadcrumb.Item>
					<Breadcrumb.Link>Writing to</Breadcrumb.Link>
				</Breadcrumb.Item>
				<Breadcrumb.Separator />
				<Breadcrumb.Item>
					<Breadcrumb.Ellipsis />
				</Breadcrumb.Item>
				<Breadcrumb.Separator />
				{#if pathSegments.length > 1}
					<Breadcrumb.Item>
						<Breadcrumb.Link class="truncate">{pathSegments[pathSegments.length - 2]}</Breadcrumb.Link>
					</Breadcrumb.Item>
					<Breadcrumb.Separator />
				{/if}
				<Breadcrumb.Item>
					<Breadcrumb.Page class="font-mono">
						{pathSegments[pathSegments.length - 1]}
					</Breadcrumb.Page>
				</Breadcrumb.Item>
			</Breadcrumb.List>
		</Breadcrumb.Root>
        {/if}
	</div>

	<!-- Right Side: Device Status -->
	<div class="flex items-center gap-2">
		{#if !deviceStore.isDeviceConnected}
			<Badge variant="destructive" class="gap-1.5">   
				<CircleX class="size-3.5" />
				Disconnected
			</Badge>
		{:else if deviceStore.isOccupied}
			<Badge variant="default" class="gap-1.5 bg-yellow-500 text-yellow-950 hover:bg-yellow-500/80">
				<LoaderCircle class="size-3.5 animate-spin" />
				Acquiring
			</Badge>
        {:else if deviceStore.isStreaming}
			<Badge variant="default" class="gap-1.5 bg-green-500 text-green-950 hover:bg-green-500/80">
				<LoaderCircle class="size-3.5 animate-spin" />
				Streaming
			</Badge>
		{:else}
			<Badge variant="default" class="gap-1.5 bg-green-500 text-green-950 hover:bg-green-500/80">
				<CircleCheck class="size-3.5" />
				Idling
			</Badge>
		{/if}
	</div>
</div>