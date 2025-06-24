<script lang="ts">
	import type { FeDeviceMeta } from '$lib/bindings/FeDeviceMeta';
    import { RadioGroupItem } from '$lib/components/ui/radio-group'
    import DeviceInfoHoverCard from '$lib/components/DeviceInfoHoverCard.svelte';
	import * as Collapsible from '$lib/components/ui/collapsible';
    import { ChevronsUpDown } from '@lucide/svelte';
    import { Checkbox } from '$lib/components/ui/checkbox';
    import { buttonVariants } from '$lib/components/ui/button';
    import { Label } from '$lib/components/ui/label';

    let {
		device,
		isSelected,
		currentSelections, // One-way prop with the selections for this item
		onSelectionChange // Callback to notify parent of a change
	} = $props<{
		device: FeDeviceMeta;
		isSelected: boolean;
		currentSelections: Set<string>;
		onSelectionChange: (route: string, isSelected: boolean) => void;
	}>();

	function handleChildSelection(checked: boolean, route:string) {
		onSelectionChange(route, checked);
	}

    const sortedChildren = $derived(
		(device.children ?? [])
			.slice()
			.sort(
				(a: FeDeviceMeta, b: FeDeviceMeta) =>
					parseInt(a.route.replace(/^\//, ''), 10) - parseInt(b.route.replace(/^\//, ''), 10)
			)
	);
</script>

<Collapsible.Root open={isSelected} class="w-full">
	<div
		class="rounded-lg border-2 border-transparent transition-all has-[:checked]:border-zinc-200"
	>
		<div class="flex items-center justify-between p-2"> <div class="flex items-center space-x-2">
				<Label for={device.url} class="flex cursor-pointer items-center space-x-3">
					<RadioGroupItem value={device.url} id={device.url} />
					<span class="font-medium">{device.name}</span>
				</Label>
				<DeviceInfoHoverCard {device} />
			</div>

			<div class="flex items-center">
				{#if sortedChildren.length > 0}
					<Collapsible.Trigger
						class={buttonVariants({ variant: 'ghost', size: 'sm', class: 'w-9 p-0' })}
					>
						<ChevronsUpDown class="h-4 w-4" />
						<span class="sr-only">Toggle stream list</span>
					</Collapsible.Trigger>
				{/if}
			</div>
		</div>
		<Collapsible.Content class="px-3 pb-3">
			<div class="ml-[2.125rem] space-y-1 border-l-2 pl-4">
				{#each sortedChildren as child (child.route)}
					<div class="flex items-center space-x-2 py-1">
						<Checkbox
                            id={`${device.url}-${child.route}`}
                            checked={currentSelections.has(child.route)}
                            onCheckedChange={(checked) => {
                                if (typeof checked === 'boolean') {
                                    handleChildSelection(checked, child.route);
                                }
                            }}
                        />
						<Label
							for={`${device.url}-${child.route}`}
							class="flex flex-grow cursor-pointer items-center space-x-3"
						>
							<span class="text-sm font-normal text-zinc-600">
								{child.route.replace(/^\//, '')}: {child.name}
							</span>
							<DeviceInfoHoverCard device={child} />
						</Label>
					</div>
				{/each}
			</div>
		</Collapsible.Content>
	</div>
</Collapsible.Root>