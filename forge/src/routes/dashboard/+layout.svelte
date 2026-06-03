<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { LogOut, LayoutDashboard, List, AppWindow, Layers } from '@lucide/svelte';
	import { page } from '$app/state';
	import * as m from '$lib/paraglide/messages';

	let { data, children } = $props();

	const user = $derived(data.user);

	function signOut() {
		window.location.href = '/auth/logout';
	}

	const navItems = $derived([
		{ href: '/dashboard', label: m.nav_overview(), icon: LayoutDashboard },
		{ href: '/dashboard/whitelist', label: m.nav_whitelist(), icon: List },
		{ href: '/dashboard/pwas', label: m.nav_pwas(), icon: AppWindow },
		{ href: '/dashboard/frontpage', label: m.nav_frontpage(), icon: Layers }
	]);
</script>

<div class="flex min-h-screen flex-col">
	<!-- Top header -->
	<header class="border-b border-border bg-background px-6 py-3">
		<div class="mx-auto flex w-full max-w-7xl items-center justify-between">
			<span class="flex items-center gap-2 align-middle">
				<img src="/arc.png" alt="Arc Forge" class="size-8" />
				<span class="mt-3 font-semibold">Arc Forge</span>
			</span>

			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					class="flex size-9 items-center justify-center overflow-hidden rounded-full bg-primary/10 ring-2 ring-transparent transition hover:ring-primary/40 focus-visible:ring-primary/40 focus-visible:outline-none"
				>
					<img src={data.avatarUrl} alt={user.name} class="size-full object-cover" />
				</DropdownMenu.Trigger>

				<DropdownMenu.Content
					class="z-50 min-w-48 rounded-lg border border-border bg-background p-1 shadow-lg"
					align="end"
					sideOffset={8}
				>
					<div class="px-3 py-2">
						<p class="text-sm font-medium">{user.name}</p>
						<p class="text-xs text-muted-foreground">{user.email}</p>
					</div>
					<DropdownMenu.Separator class="my-1 h-px bg-border" />
					<DropdownMenu.Item
						class="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm text-destructive hover:bg-destructive/10 focus:bg-destructive/10 focus:outline-none"
						onSelect={signOut}
					>
						<LogOut class="size-4" />
						{m.sign_out()}
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>
	</header>

	<div class="mx-auto flex w-full max-w-7xl flex-1 gap-8 px-6 py-8">
		<!-- Sidebar -->
		<nav class="w-48 shrink-0">
			<ul class="space-y-1">
				{#each navItems as item (item.href)}
					{@const active =
						page.url.pathname === item.href ||
						(item.href !== '/dashboard' && page.url.pathname.startsWith(item.href))}
					<li>
						<a
							href={item.href}
							class="flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors {active
								? 'bg-primary/10 font-medium text-primary'
								: 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
						>
							<item.icon class="size-4 shrink-0" />
							{item.label}
						</a>
					</li>
				{/each}
			</ul>
		</nav>

		<!-- Page content -->
		<main class="min-w-0 flex-1">
			{@render children()}
		</main>
	</div>
</div>
