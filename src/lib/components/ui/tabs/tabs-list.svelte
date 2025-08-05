<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";
	import type { Snippet } from "svelte";

	type Props = TabsPrimitive.ListProps & {
		children: Snippet;
	};

	let {
		ref: listEl = $bindable(null),
		class: className,
		children,
		...restProps
	}: Props = $props();

	let underlineStyle = $state({ left: "0px", width: "0px", opacity: "0" });

	const updateUnderline = () => {
		if (!listEl) return;
		const activeTrigger = listEl.querySelector<HTMLElement>(
			`[data-slot="tabs-trigger"][data-state="active"]`
		);
		if (activeTrigger) {
			underlineStyle = {
				left: `${activeTrigger.offsetLeft}px`,
				width: `${activeTrigger.offsetWidth}px`,
				opacity: "1",
			};
		}
	};

	$effect(() => {
		if (!listEl) return;
		updateUnderline();

		const observer = new MutationObserver(() => {
			updateUnderline();
		});

		observer.observe(listEl, {
			attributes: true,
			childList: true,
			subtree: true,
			attributeFilter: ["data-state"],
		});

		return () => observer.disconnect();
	});
</script>

<TabsPrimitive.List
	bind:ref={listEl}
	data-slot="tabs-list"
	class={cn("relative flex items-center justify-start border-b", className)}
	{...restProps}
>
	{@render children?.()}
	<div
		data-slot="underline"
		class="absolute bottom-[-1px] h-0.5 bg-primary transition-all duration-300 ease-in-out"
		style:left={underlineStyle.left}
		style:width={underlineStyle.width}
		style:opacity={underlineStyle.opacity}
	></div>
</TabsPrimitive.List>