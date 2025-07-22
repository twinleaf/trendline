<script lang="ts">
	import { chartState } from '$lib/states/chartState.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Separator } from '$lib/components/ui/separator';
	import { Plus, ChevronDown, Trash2, AlignVerticalDistributeCenter } from '@lucide/svelte';

	let isConfirmDialogOpen = $state(false);

	function confirmDeleteAll() {
		chartState.deleteAllPlots();
		isConfirmDialogOpen = false;
	}
</script>

<div class="flex items-center rounded-lg bg-card shadow-lg">
	<!-- Main Action Button -->
	<Button
		class="h-10 rounded-r-none border-r-0"
		onclick={() => chartState.addPlot()}
		aria-label="Add Plot"
	>
		<Plus class="mr-2 h-4 w-4" />
		Add Plot
	</Button>

	<Separator orientation="vertical" class="h-10" />

	<!-- Dropdown Menu for Secondary Actions -->
	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			<Button
				variant="default"
				size="icon"
				class="h-10 w-9 rounded-l-none border-l-0"
				aria-label="More plot options"
			>
				<ChevronDown class="h-4 w-4" />
			</Button>
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="end" class="w-56">
			<DropdownMenu.Label>More Actions</DropdownMenu.Label>
			<DropdownMenu.Separator />
			<DropdownMenu.Item onclick={() => chartState.rebalancePlots()}>
				<AlignVerticalDistributeCenter class="mr-2 h-4 w-4" />
				<span>Rebalance Plots</span>
			</DropdownMenu.Item>
			<DropdownMenu.Item
				onclick={() => (isConfirmDialogOpen = true)}
				class="text-destructive focus:text-destructive"
			>
				<Trash2 class="mr-2 h-4 w-4" />
				<span>Delete All Plots</span>
			</DropdownMenu.Item>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
</div>

<!-- Confirmation Dialog for Deleting All Plots -->
<AlertDialog.Root bind:open={isConfirmDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
			<AlertDialog.Description>
				This action cannot be undone. This will permanently delete all plots and their configurations.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel onclick={() => (isConfirmDialogOpen = false)}>Cancel</AlertDialog.Cancel>
			<AlertDialog.Action
				class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
				onclick={confirmDeleteAll}
			>
				Delete All
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>