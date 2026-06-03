<script lang="ts">
	import { enhance } from '$app/forms';
	import { Trash2, Pencil, Plus } from '@lucide/svelte';
	import { Button, buttonVariants } from '$lib/components/ui/button/index.js';
	import * as m from '$lib/paraglide/messages';

	let { data } = $props();
</script>

<svelte:head>
	<title>PWAs - Arc Forge</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h2 class="text-lg font-semibold">{m.pwas_heading()}</h2>
			<p class="text-sm text-muted-foreground">
				{m.pwas_served_public()} <code class="rounded bg-muted px-1 py-0.5 font-mono text-xs"
					>/api/pwas</code
				>.
			</p>
		</div>
		<!-- eslint-disable svelte/no-navigation-without-resolve -->
		<a href="/dashboard/pwas/new" class={buttonVariants()}>
			<Plus class="size-4" />
			{m.pwas_new()}
		</a>
	</div>

	{#if data.apps.length === 0}
		<p class="text-sm text-muted-foreground">{m.pwas_empty()}</p>
	{:else}
		<ul class="divide-y divide-border rounded-lg border border-border">
			{#each data.apps as app (app.id)}
				<li class="flex items-center justify-between px-4 py-3">
					<div class="flex items-center gap-3">
						<img src={app.iconUrl} alt={app.name} class="size-8 rounded" />
						<div>
							<p class="text-sm font-medium">{app.name}</p>
							<p class="text-xs text-muted-foreground">{app.appid}</p>
						</div>
					</div>
					<div class="flex items-center gap-1">
						<a
							href="/dashboard/pwas/{app.id}"
							class={buttonVariants({ variant: 'ghost', size: 'icon' })}
						>
							<Pencil class="size-4" />
						</a>
						<form method="POST" action="?/delete" use:enhance>
							<input type="hidden" name="id" value={app.id} />
							<Button
								type="submit"
								variant="ghost"
								size="icon"
								class="text-muted-foreground hover:text-destructive"
							>
								<Trash2 class="size-4" />
							</Button>
						</form>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>
