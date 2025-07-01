<script lang="ts">
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Collapsible from '$lib/components/ui/collapsible';
	import { Label } from '$lib/components/ui/label';
	import { ChevronsUpDown }   from '@lucide/svelte';
	import DeviceInfoHoverCard  from '$lib/components/device-select/DeviceInfoHoverCard.svelte';
	import type { UiDevice } from '$lib/bindings/UiDevice';

	type DeviceWithChildren = UiDevice & { children: UiDevice[] };

	let {
		devices,
		selectedParent = $bindable(),
		childrenSelections = $bindable()
	} = $props<{
		devices: DeviceWithChildren[];
		selectedParent: string;
		childrenSelections: Map<string, Set<string>>;
	}>();

	$effect(() => {
		const parentUrl = selectedParent;
		if (!parentUrl) return;


		if (!childrenSelections.has(parentUrl)) {
			const parentDevice = devices.find((d: { url: any; }) => d.url === parentUrl);
			if (parentDevice) {
				const allChildrenRoutes = new Set(parentDevice.children.map((c: { route: any; }) => c.route));

				const newSelections = new Map(childrenSelections);
				newSelections.set(parentUrl, allChildrenRoutes);
				childrenSelections = newSelections;
			}
		}
	});

	function toggleChild(childRoute: string, isChecked: boolean, parentUrl: string) {
		const existingChildren = childrenSelections.get(parentUrl);

		if (existingChildren) {
			if (isChecked) {
				existingChildren.add(childRoute);
			} else {
				existingChildren.delete(childRoute);
			}
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
			const currentIndex = devices.findIndex((d: { url: any; }) => d.url === selectedParent);
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
					<RadioGroup.Item id={root.url} value={root.url} tabindex={-1} />
					<Label for={root.url} class="font-medium cursor-pointer">
						{root.meta.name}
					</Label>
					<DeviceInfoHoverCard device={root} />
				</div>
				{#if root.children.length}
					<Collapsible.Trigger
						class="w-8 h-8 flex items-center justify-center rounded-md"
						aria-label="Toggle children"
					>
						<ChevronsUpDown class="h-4 w-4" />
					</Collapsible.Trigger>
				{/if}
			</div>

			<Collapsible.Content class="pl-6 space-y-1">
				{#each root.children as child (child.url + child.route)}
					<div class="flex items-center space-x-2 py-0.5">
						<Checkbox
							id={child.url + child.route}
							checked={childrenSelections.get(root.url)?.has(child.route) ?? false}
							onCheckedChange={(v) => toggleChild(child.route, !!v, root.url)}
						/>
						<Label for={child.url + child.route} class="cursor-pointer text-sm">
							{child.route.slice(1)}: {child.meta.name}
						</Label>
						<DeviceInfoHoverCard device={child} />
					</div>
				{/each}
			</Collapsible.Content>
		</Collapsible.Root>
	{/each}
</RadioGroup.Root>