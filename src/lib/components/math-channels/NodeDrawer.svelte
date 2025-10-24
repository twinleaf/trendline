<script lang="ts">
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';
	import { buttonVariants } from '$lib/components/ui/button/index.js';
	import { ChevronsUpDown } from '@lucide/svelte';
	import {
		draggable,
		events,
		position,
		Compartment,
		type DragEventData
	} from '@neodrag/svelte';
	import { useSvelteFlow, useNodes, type Node, type XYPosition } from '@xyflow/svelte';
	import { onMount, onDestroy } from 'svelte';

	const { screenToFlowPosition } = useSvelteFlow();
	const nodesStore = useNodes();

	let lastPointerPos = $state({ x: 0, y: 0 });
	const handlePointerMove = (e: PointerEvent) => {
		lastPointerPos = { x: e.clientX, y: e.clientY };
	};
	onMount(() => {
		window.addEventListener('pointermove', handlePointerMove);
	});
	onDestroy(() => {
		window.removeEventListener('pointermove', handlePointerMove);
	});
	let snapbackPos = $state({ x: 0, y: 0 });
	const posComp = Compartment.of(() => position({ current: snapbackPos }));
	// --- End unchanged logic ---

	const CLICK_THRESHOLD = 5;
	const CASCADE_OFFSET = 5;

	/**
	 * Checks if a position is occupied and finds the next available "cascaded" spot.
	 */
	function getOffsetPosition(targetPos: XYPosition, allNodes: Node[]): XYPosition {
		const collision = allNodes.find(
			(node) => node.position.x === targetPos.x && node.position.y === targetPos.y
		);

		if (collision) {
			const newTargetPos = {
				x: targetPos.x + CASCADE_OFFSET,
				y: targetPos.y + CASCADE_OFFSET
			};
			return getOffsetPosition(newTargetPos, allNodes);
		}

		return targetPos;
	}

	const handleDragEnd = (data: DragEventData, nodeType: string) => {
		const dragDistance = Math.sqrt(data.offset.x ** 2 + data.offset.y ** 2);
		const isClick = dragDistance < CLICK_THRESHOLD;

		let initialPosition: XYPosition;

		const flowPane = document.querySelector<HTMLDivElement>('.svelte-flow__pane');
		if (!flowPane) {
			snapbackPos = { x: 0, y: 0 };
			return;
		}

		if (isClick) {
			const flowRect = flowPane.getBoundingClientRect();
			const centerX = flowRect.left + flowRect.width / 2;
			const centerY = flowRect.top + flowRect.height / 2;
			initialPosition = screenToFlowPosition({ x: centerX, y: centerY });
		} else {
			const { x, y } = lastPointerPos;
			const flowRect = flowPane.getBoundingClientRect();

			const isOverFlow =
				x >= flowRect.left &&
				x <= flowRect.right &&
				y >= flowRect.top &&
				y <= flowRect.bottom;

			if (!isOverFlow) {
				snapbackPos = { x: 0, y: 0 };
				return;
			}
			initialPosition = screenToFlowPosition({ x, y });
		}

		const allNodes = nodesStore.current;

		const finalPosition = getOffsetPosition(initialPosition, allNodes);

		const newNode: Node = {
			id: `${crypto.randomUUID?.()}`,
			type: nodeType,
			position: finalPosition,
			data: { label: `${nodeType} node` },
			origin: [0.5, 0.0]
		};

		nodesStore.update((currentNodes) => [...currentNodes, newNode]);
		snapbackPos = { x: 0, y: 0 };
	};
</script>

<div class="pointer-events-auto">
	<Collapsible.Root>
		<div class="flex justify-center">
			<Collapsible.Trigger
				class={`${buttonVariants({ variant: 'secondary', size: 'sm' })} rounded-full shadow-md`}
				aria-label="Toggle node palette"
			>
				<ChevronsUpDown class="mr-2 h-4 w-4" />
				Nodes
			</Collapsible.Trigger>
		</div>
		<Collapsible.Content class="mt-2 flex justify-center">
			<div
				class="rounded-xl border bg-popover text-popover-foreground shadow-lg
backdrop-blur p-3"
			>
				<div class="grid grid-cols-[auto_auto_auto] gap-4">
					<section class="flex flex-col items-center">
						<h4 class="text-[11px] uppercase tracking-wide opacity-70 mb-1">Sources</h4>
						<div class="flex flex-wrap gap-2 justify-center">
							<div
								class="select-none cursor-grab rounded border px-3 py-1 text-xs font-medium hover:bg-muted"
								{@attach
									draggable(() => [
										posComp,
										events({
											onDragEnd: (data) => handleDragEnd(data, 'input')
										})
									])}
							>
								Input
							</div>
						</div>
					</section>

					<section class="flex flex-col items-center">
						<h4 class="text-[11px] uppercase tracking-wide opacity-70 mb-1">Transforms</h4>
						<div class="flex flex-wrap gap-2 justify-center">
							<div
								class="select-none cursor-grab rounded border px-3 py-1 text-xs font-medium hover:bg-muted"
								{@attach
									draggable(() => [
										posComp,
										events({
											onDragEnd: (data) => handleDragEnd(data, 'default')
										})
									])}
							>
								Default
							</div>
						</div>
					</section>

					<section class="flex flex-col items-center">
						<h4 class="text-[11px] uppercase tracking-wide opacity-70 mb-1">Sinks</h4>
						<div class="flex flex-wrap gap-2 justify-center">
							<div
								class="select-none cursor-grab rounded border px-3 py-1 text-xs font-medium hover:bg-muted"
								{@attach
									draggable(() => [
										posComp,
										events({
											onDragEnd: (data) => handleDragEnd(data, 'output')
										})
									])}
							>
								Output
							</div>
						</div>
					</section>
				</div>
			</div>
		</Collapsible.Content>
	</Collapsible.Root>
</div>