<script lang="ts">
	import * as m from '$lib/paraglide/messages';
</script>

<svelte:head>
	<title>Dashboard - Arc Forge</title>
</svelte:head>

<h2 class="text-lg font-semibold">{m.nav_overview()}</h2>
<p class="mt-1 text-sm text-muted-foreground">{m.dashboard_welcome()}</p>
