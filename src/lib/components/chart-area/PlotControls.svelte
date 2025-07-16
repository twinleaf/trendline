<script lang="ts">
import type { PlotConfig } from '$lib/states/chartState.svelte';
import type { DecimationMethod } from '$lib/bindings/DecimationMethod';


import type { ExpandedState } from '@tanstack/table-core';
import { columns, type TreeRow } from '$lib/components/chart-area/data-table/column';
import DataTable from '$lib/components/chart-area/data-table/DataTable.svelte';
import PlotSettings from '$lib/components/chart-area/PlotSettings.svelte';


import { Button } from '$lib/components/ui/button/';
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
		<Tabs.Root value="selection" class="w-full p-2">
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
				<PlotSettings bind:plot />
			</Tabs.Content>
		</Tabs.Root>
	</Popover.Content>
</Popover.Root>