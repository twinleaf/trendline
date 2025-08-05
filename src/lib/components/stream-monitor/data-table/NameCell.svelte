<script lang="ts">
	let {
		name,
		depth = 0,
		isGroup = false
	}: {
		name: string;
		depth?: number;
		isGroup?: boolean;
	} = $props();

	const paddingLeft = `${depth * 1.25}rem`;
</script>

<div
	style="padding-left: {paddingLeft};"
	class="flex h-full items-center"
	class:font-semibold={isGroup}
	class:text-muted-foreground={isGroup}
>
	{name}
</div>