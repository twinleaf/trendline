<script lang="ts">
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { Label } from '$lib/components/ui/label';
	import { ChevronsUpDown }   from '@lucide/svelte';
	import DeviceInfoHoverCard  from '$lib/components/device-select/DeviceInfoHoverCard.svelte';
	import type { UiDeviceWithKids } from '$lib/states/deviceState.svelte';


	let {
		devices,
		selectedParent = $bindable(),
		childrenSelections = $bindable()
	} = $props<{
		devices: UiDeviceWithKids[];
		selectedParent: string;
		childrenSelections: Map<string, Set<string>>;
	}>();

	function toggleChild(childRoute: string, isChecked: boolean, parentRoute: string) {
		const children = childrenSelections.get(parentRoute);
		if (!children) return;

		if (isChecked) {
			children.add(childRoute);
		} else {
			children.delete(childRoute);
		}
	}


	function handleKeyDown(event: KeyboardEvent) {
		const form = (event.currentTarget as HTMLElement).closest('form');

		// Handle Enter to submit the form
		if (event.key === 'Enter') {
			event.preventDefault();
			form?.requestSubmit();
			return; // Stop further processing
		}

		// Handle Up/Down arrow keys for parent navigation
		if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
			event.preventDefault();
			const currentIndex = devices.findIndex((d: { route: any; }) => d.route === selectedParent);
			const maxIndex = devices.length - 1;
			let nextIndex = -1;

			if (event.key === 'ArrowDown') {
				nextIndex = currentIndex >= maxIndex ? 0 : currentIndex + 1;
			} else {
				nextIndex = currentIndex <= 0 ? maxIndex : currentIndex - 1;
			}

			if (nextIndex !== -1 && devices[nextIndex]) {
				selectedParent = devices[nextIndex].route;
			}
		}
	}
</script>

<RadioGroup.Root
	bind:value={selectedParent}
	orientation="vertical"
	onkeydown={handleKeyDown}
	class="relative focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-md"
	tabindex={0}
>
	{#each devices as root (root.url)}
		<Collapsible.Root class="w-full">
			<div class="flex items-center justify-between px-2 py-1.5">
				<div class="flex items-center space-x-2">
					<!-- The RadioGroup.Item is now just a visual indicator -->
					<RadioGroup.Item id={root.url} value={root.route} tabindex={-1} />
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
							checked={childrenSelections.get(root.route)?.has(child.route) ?? false}
							onCheckedChange={(v) => toggleChild(child.route, !!v, root.route)}
						/>
						<Label for={child.route} class="cursor-pointer text-sm">
							{child.route.slice(1)}: {child.meta.name}
						</Label>
						<DeviceInfoHoverCard device={child} />
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</RadioGroup.Root>