<script lang="ts">
	import { enhance } from '$app/forms';
	import { Trash2 } from '@lucide/svelte';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Button } from '$lib/components/ui/button/index.js';

	let { data } = $props();

	let newValue = $state('');
</script>

<svelte:head>
	<title>Lutris Whitelist - Arc Forge</title>
</svelte:head>

<div class="space-y-6">
	<div>
		<h2 class="text-lg font-semibold">Lutris Whitelist</h2>
		<p class="text-sm text-muted-foreground">
			Entries are served publicly at <code class="rounded bg-muted px-1 py-0.5 font-mono text-xs">/api/lutris-whitelist</code>.
		</p>
	</div>

	<form
		method="POST"
		action="?/add"
		use:enhance={() => async ({ update }) => {
			await update();
			newValue = '';
		}}
		class="flex gap-2"
	>
		<Input name="value" bind:value={newValue} placeholder="runner:game-id" class="flex-1" required />
		<Button type="submit">Add</Button>
	</form>

	{#if data.entries.length === 0}
		<p class="text-sm text-muted-foreground">No entries yet.</p>
	{:else}
		<ul class="divide-y divide-border rounded-lg border border-border">
			{#each data.entries as entry (entry.id)}
				<li class="flex items-center justify-between px-4 py-2.5">
					<code class="font-mono text-sm">{entry.value}</code>
					<form method="POST" action="?/remove" use:enhance>
						<input type="hidden" name="id" value={entry.id} />
						<Button type="submit" variant="ghost" size="icon" class="text-muted-foreground hover:text-destructive">
							<Trash2 class="size-4" />
						</Button>
					</form>
				</li>
			{/each}
		</ul>
	{/if}
</div>
