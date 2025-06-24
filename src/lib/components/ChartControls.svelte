<script lang="ts">
    import { Button } from '$lib/components/ui/button';
	let { 
        isAutoScrolling = $bindable(), 
        viewStartTime = $bindable(), 
        endStartTime = $bindable() 
        } = $props()
</script>

<div class="controls">
	<Button onclick={() => (isAutoScrolling = !isAutoScrolling)}>
		Toggle Auto-Scroll ({isAutoScrolling ? 'On' : 'Off'})
    </Button>
	<Button onclick={() => (isAutoScrolling = true)} disabled={isAutoScrolling}>
		Go to Live View
    </Button>
</div>

<style>
    .controls {
		padding: 1rem;
		margin-bottom: 1rem;
		border: 1px solid oklch(var(--border));
		border-radius: var(--radius);
		display: flex;
		gap: 1rem;
		align-items: center;
		flex-wrap: wrap;
		justify-content: center;
	}
</style>