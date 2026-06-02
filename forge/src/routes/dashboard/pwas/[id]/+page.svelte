<script lang="ts">
	import PwaForm from '../PwaForm.svelte';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.app.name} - Arc Forge</title>
</svelte:head>

<div class="space-y-6">
	<h2 class="text-lg font-semibold">Edit {data.app.name}</h2>
	<form method="POST">
		<PwaForm values={data.app} submitLabel="Save Changes" />
	</form>
</div>
