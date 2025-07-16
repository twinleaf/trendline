<script lang="ts">
	import { Funnel, ChevronDown } from '@lucide/svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import Button from '$lib/components/ui/button/button.svelte';
    import { SvelteSet } from 'svelte/reactivity';
	
	interface Props {
		allPrefixes: string[];
		selected: SvelteSet<string>;
	}

	let { allPrefixes, selected = $bindable() }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger>
		<Button variant="outline" class="h-9">
			<Funnel class="size-4" />
			<ChevronDown class="ml-2 size-4" />
		</Button>
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end">
		<DropdownMenu.Label>Filter by Prefix</DropdownMenu.Label>
		<DropdownMenu.Separator />
		{#each allPrefixes as prefix}
			<DropdownMenu.CheckboxItem
				checked={selected.has(prefix)}
				onCheckedChange={(isChecked) => {
					if (isChecked) {
						selected.add(prefix);
					} else {
						selected.delete(prefix);
					}
				}}
			>
				{prefix}
			</DropdownMenu.CheckboxItem>
		{/each}
	</DropdownMenu.Content>
</DropdownMenu.Root>