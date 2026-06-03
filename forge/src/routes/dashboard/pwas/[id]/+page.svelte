<script lang="ts">
	import PwaForm from '../PwaForm.svelte';
	import * as m from '$lib/paraglide/messages';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.app.name} - Arc Forge</title>
</svelte:head>

<div class="space-y-6">
	<h2 class="text-lg font-semibold">{m.pwa_edit_heading()} {data.app.name}</h2>
	<form method="POST">
		<PwaForm values={data.app} submitLabel={m.pwa_save_changes()} />
	</form>
</div>
