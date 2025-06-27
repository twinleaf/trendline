// src/lib/components/StatusFooter.svelte

<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import * as Breadcrumb from '$lib/components/ui/breadcrumb';
	import { CircleCheck, CircleX, LoaderCircle } from '@lucide/svelte';
	import { ioStore } from '$lib/stores/io.store.svelte';
	import { deviceService } from '$lib/stores/device.store.svelte';

	const pathSegments = $derived(ioStore.loggingPath.split('/').filter(Boolean));
</script>

<div
	class="flex h-9 items-center justify-between rounded-md border bg-card p-1 text-sm shadow-xs"
>
	<!-- Left Side: Activity / Logging Path (Unchanged) -->
	<div class="min-w-0 flex-1 items-center gap-4">
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
							<Breadcrumb.Link class="truncate"
								>{pathSegments[pathSegments.length - 2]}</Breadcrumb.Link
							>
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
		{#if deviceService.selectedPortState() === 'Connecting'
            || deviceService.selectedPortState() === 'Discovery'
            || deviceService.selectedPortState() === 'Reconnecting'}
            <Badge variant="default" class="gap-1.5 bg-yellow-500 text-yellow-950 hover:bg-yellow-500/80">
                <LoaderCircle class="size-3.5 animate-spin" />
                Acquiring…
            </Badge>

        {:else if deviceService.selectedPortState() === 'Disconnected'}
            <Badge variant="destructive" class="gap-1.5">
                <CircleX class="size-3.5" />
                Disconnected
            </Badge>

        {:else if deviceService.selectedPortState() === 'Streaming'}
            <Badge variant="default" class="gap-1.5 bg-green-500 text-green-950 hover:bg-green-500/80">
                <LoaderCircle class="size-3.5 animate-spin" />
                Streaming
            </Badge>

        {:else if deviceService.selectedPortState() === 'Idle'}
            <Badge variant="default" class="gap-1.5 bg-yellow-500 text-yellow-950 hover:bg-yellow-500/80">
                <CircleCheck class="size-3.5" />
                Idle
            </Badge>
        {/if}
	</div>
</div>