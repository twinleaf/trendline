<script lang="ts">
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { Label } from '$lib/components/ui/label';
	import { ChevronsUpDown }   from '@lucide/svelte';
	import DeviceInfoHoverCard  from '$lib/components/device-select/DeviceInfoHoverCard.svelte';
	import type { UiDeviceWithKids } from '$lib/stores/device.store.svelte';

	type Selection = {
        parent: string;
        children: Set<string>;
    };

	let { devices, selection = $bindable() } = $props<{
		devices: UiDeviceWithKids[],
		selection: Selection
	}>();

	function toggleChild(childRoute: string, isChecked: boolean, parentRoute: string) {
		if (isChecked) {
			selection.parent = parentRoute;
			selection.children.add(childRoute);
		} else {
			selection.children.delete(childRoute);
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			const form = (event.currentTarget as HTMLElement).closest('form');
			form?.requestSubmit();
		}
	}

</script>

<RadioGroup.Root bind:value={selection.parent} orientation="vertical">
	{#each devices as root (root.url)}
		<Collapsible.Root class="w-full">
			<div class="flex items-center justify-between px-2 py-1.5">
				<div class="flex items-center space-x-2">
					<RadioGroup.Item
						id={root.url}
						value={root.route}
						onkeydown={handleKeyDown}
					/>
					<Label for={root.url} class="font-medium cursor-pointer">
						{root.meta.name}
					</Label>
					<DeviceInfoHoverCard device={root} />
				</div>
				{#if root.childrenSorted.length}
					<Collapsible.Trigger
						class="w-8 h-8 flex items-center justify-center rounded-md"
						aria-label="Toggle children"
					>
						<ChevronsUpDown class="h-4 w-4" />
					</Collapsible.Trigger>
				{/if}
			</div>

			<Collapsible.Content class="pl-6 space-y-1">
				{#each root.childrenSorted as child (child.route)}
					<div class="flex items-center space-x-2 py-0.5">
						<Checkbox
							id={child.route}
							checked={selection.children.has(child.route)}
							onCheckedChange={(v) => toggleChild(child.route, !!v, root.route)}
						/>
						<Label for={child.route} class="flex-1 cursor-pointer text-sm">
							{child.route.slice(1)}: {child.meta.name}
						</Label>
						<DeviceInfoHoverCard device={child} />
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</RadioGroup.Root>