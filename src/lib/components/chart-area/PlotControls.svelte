<script lang="ts">
import type { PlotConfig } from '$lib/states/chartState.svelte';
import type { DecimationMethod } from '$lib/bindings/DecimationMethod';


import type { ExpandedState } from '@tanstack/table-core';
import { columns, type TreeRow } from '$lib/components/chart-area/data-table/column';
import DataTable from '$lib/components/chart-area/data-table/DataTable.svelte';

import { Button } from '$lib/components/ui/button/';
import { Label } from '$lib/components/ui/label';
import { RadioGroup, RadioGroupItem } from '$lib/components/ui/radio-group';
import * as Popover from '$lib/components/ui/popover';
import * as Tabs from '$lib/components/ui/tabs';

import { Settings } from '@lucide/svelte';


type Props = {
    plot: PlotConfig;
    treeData: TreeRow[];
    initialExpanded: ExpandedState;
};

let { plot = $bindable(), treeData, initialExpanded }: Props = $props();

const decimationMethods: { value: DecimationMethod; label: string; description: string }[] = [
    { value: 'None', label: 'None', description: 'No downsampling. Raw data is rendered.' },
    { value: 'MinMax', label: 'Min/Max', description: 'Selects the min and max values for each bucket. Best for multiple series on a single plot.' },
    { value: 'Fpcs', label: 'FPCS', description: 'Fast Point-Cloud Simplification. Highest visual fidelity. Best for single series.' }
];
</script>
<Popover.Root>
	<Popover.Trigger>
		<Button variant="ghost" size="icon" aria-label="Plot settings">
			<Settings class="size-5 text-muted-foreground" />
		</Button>
	</Popover.Trigger>
	<Popover.Content class="w-[600px] p-0">
		<Tabs.Root value="selection" class="w-full">
			<Tabs.List class="grid w-full grid-cols-2">
				<Tabs.Trigger value="selection">Plot Selection</Tabs.Trigger>
				<Tabs.Trigger value="settings">Plot Settings</Tabs.Trigger>
			</Tabs.List>
			<Tabs.Content value="selection">
				<div class="flex max-h-[50vh] flex-col overflow-y-auto p-2">
					<DataTable
						{columns}
						data={treeData}
						getSubRows={(row: TreeRow) => row.subRows}
						initialExpanded={initialExpanded}
						bind:rowSelection={plot.rowSelection}
					/>
				</div>
			</Tabs.Content>
			<Tabs.Content value="settings">
				<div class="max-h-[50vh] space-y-4 overflow-y-auto p-4">
					<h4 class="font-medium leading-none">Decimation</h4>
					<p class="text-sm text-muted-foreground">
						Select a downsampling method to change visual fidelity and plotting performance.
					</p>
					<RadioGroup bind:value={plot.decimationMethod} class="grid gap-2">
						{#each decimationMethods as method}
							<Label
								class="flex cursor-pointer items-start gap-3 rounded-md border p-3 hover:bg-accent hover:text-accent-foreground has-[:checked]:bg-accent"
							>
								<RadioGroupItem value={method.value} id={method.value} />
								<div class="grid gap-1.5 leading-normal">
									<span class="font-semibold">{method.label}</span>
									<p class="text-sm text-muted-foreground">{method.description}</p>
								</div>
							</Label>
						{/each}
					</RadioGroup>
				</div>
			</Tabs.Content>
		</Tabs.Root>
	</Popover.Content>
</Popover.Root>