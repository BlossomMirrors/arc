<script lang="ts">
	import { page } from '$app/state';
	import { locales, localizeHref } from '$lib/paraglide/runtime';
	import './layout.css';
	import { ModeWatcher } from 'mode-watcher';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href="/favicon.ico" sizes="any" />
	<link rel="icon" href="/favicon.svg" type="image/svg+xml" />
	<link rel="apple-touch-icon" href="/apple-touch-icon.png" />
	<link rel="manifest" href="/manifest.webmanifest" />
	<meta name="theme-color" content="#0c0c12" />
	<script
		defer
		src="https://stats.blossomos.org/script.js"
		data-website-id="d2b9026b-0b3e-4eac-89b3-19f06fc5bad3"
	></script>

	<meta
		name="keywords"
		content="open source operating system, linux distribution, blossomos, privacy os, secure linux, community driven os, custom linux iso, linux for creators, open source community"
	/>

	<!-- Open Graph -->
	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="BlossomOS" />
	<meta property="og:url" content={page.url.href} />
	<meta property="og:image" content="https://blossomos.org/cover.png" />
	<meta property="og:image:width" content="2576" />
	<meta property="og:image:height" content="1456" />
	<meta property="og:image:type" content="image/png" />

	<!-- X (Twitter) -->
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:url" content={page.url.href} />
	<meta name="twitter:image" content="https://blossomos.org/cover.png" />
</svelte:head>

<ModeWatcher />

<div class="flex items-center justify-between">
	<div class="mx-auto w-full max-w-7xl">
		<main class="px-4">{@render children()}</main>
	</div>
</div>

<!-- eslint-disable -->
<div style="display:none">
	{#each locales as locale}
		<a href={localizeHref(page.url.pathname, { locale })}>
			{locale}
		</a>
	{/each}
</div>
