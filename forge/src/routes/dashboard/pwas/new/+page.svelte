<script lang="ts">
	import PwaForm from '../PwaForm.svelte';
	import * as m from '$lib/paraglide/messages';
</script>

<svelte:head>
	<title>New PWA - Arc Forge</title>
</svelte:head>

<div class="space-y-6">
	<h2 class="text-lg font-semibold">{m.pwa_new_heading()}</h2>
	<form method="POST">
		<PwaForm submitLabel={m.pwa_create()} />
	</form>
</div>
