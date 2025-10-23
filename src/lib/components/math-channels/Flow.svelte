<script lang="ts">
  import {
    SvelteFlow,
    Controls,
    Background,
    BackgroundVariant,
    useSvelteFlow,
    type Node,
    Panel,
    type OnBeforeDelete,
    getIncomers,
    getOutgoers,
    getConnectedEdges,
    type Edge,
  } from '@xyflow/svelte';

  import NodeDrawer from './NodeDrawer.svelte';

  import '@xyflow/svelte/dist/style.css';
    import { useDnD } from './DnDProvider.svelte';

  let nodes = $state.raw<Node[]>([]);
  let edges = $state.raw<Edge[]>([]);

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
            }))
            .filter((e) => !existing.has(`${e.source}->${e.target}`))
        );

      remainingNodes = remainingNodes.filter((rn) => rn.id !== node.id);
      return [...remainingEdges, ...createdEdges];
    }, edges);

    nodes = remainingNodes;
    return true;
  };

  const { screenToFlowPosition } = useSvelteFlow();
  const type = useDnD();
  
  const onDragOver = (event: DragEvent) => {
    console.log("On drag over");
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'move';
    }
  };

  const onDrop = (event: DragEvent) => {
    console.log("On drop");
    event.preventDefault();

    if (!type.current) return;

    const position = screenToFlowPosition({
      x: event.clientX,
      y: event.clientY,
    });

    const newNode: Node = {
      id: `${crypto.randomUUID?.()}`,
      type: type.current,
      position,
      data: { label: `${type.current} node` },
      origin: [0.5, 0.0],
    };

    nodes = [...nodes, newNode];
  };
</script>

<div class="h-full">
  <SvelteFlow
    bind:nodes
    bind:edges
    fitView
    {onbeforedelete}
    ondragover={onDragOver}
    ondrop={onDrop}
  >
    <Panel position="top-center" class="pointer-events-none z-50">
      <NodeDrawer />
    </Panel>

    <Controls orientation="horizontal"/>
    <Background variant={BackgroundVariant.Lines} />
  </SvelteFlow>
</div>