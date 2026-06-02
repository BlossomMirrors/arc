<script lang="ts">
	import PwaForm from '../PwaForm.svelte';
</script>

<svelte:head>
	<title>New PWA - Arc Forge</title>
</svelte:head>

<div class="space-y-6">
	<h2 class="text-lg font-semibold">New PWA</h2>
	<form method="POST">
		<PwaForm submitLabel="Create" />
	</form>
</div>
