<script lang="ts">
	import {
		SvelteFlow,
		Controls,
		Background,
		BackgroundVariant,
		type Node,
		Panel,
		type OnBeforeDelete,
		getIncomers,
		getOutgoers,
		getConnectedEdges,
		type Edge,
    type SnapGrid,
    MarkerType
	} from '@xyflow/svelte';
	import NodeDrawer from './NodeDrawer.svelte';
	import '@xyflow/svelte/dist/style.css';

	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);

  const defaultEdgeOptions = {
		markerEnd: { type: MarkerType.ArrowClosed }
	};

	const onbeforedelete: OnBeforeDelete = async ({ nodes: deletedNodes }) => {
		let remainingNodes = [...nodes];
		edges = deletedNodes.reduce((acc, node) => {
			const incomers = getIncomers(node, remainingNodes, acc);
			const outgoers = getOutgoers(node, remainingNodes, acc);
			const connectedEdges = getConnectedEdges([node], acc);
			const remainingEdges = acc.filter((e) => !connectedEdges.includes(e));
			const existing = new Set(remainingEdges.map((e) => `${e.source}->${e.target}`));
			const createdEdges = incomers
				.flatMap(({ id: source }) =>
					outgoers
						.map(({ id: target }) => ({
							id: `reconnect:${source}->${target}:${crypto.randomUUID?.() ?? Math.random()}`,
							source,
							target,
              markerEnd: { type: MarkerType.ArrowClosed }
						}))
						.filter((e) => !existing.has(`${e.source}->${e.target}`))
				);
			remainingNodes = remainingNodes.filter((rn) => rn.id !== node.id);
			return [...remainingEdges, ...createdEdges];
		}, edges);
		nodes = remainingNodes;
		return true;
	};

</script>

<div class="h-full">
	<SvelteFlow
		bind:nodes
		bind:edges
		fitView
    snapGrid = {[20, 20] satisfies SnapGrid}
		{onbeforedelete}
    {defaultEdgeOptions}
		>
		<Panel position="top-center" class="pointer-events-none z-50">
			<NodeDrawer />
		</Panel>
		<Controls orientation="horizontal" />
		<Background variant={BackgroundVariant.Lines} />
	</SvelteFlow>
</div>