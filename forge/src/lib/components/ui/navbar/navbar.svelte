<script lang="ts" module>
	import type { Component } from 'svelte';

	export type NavDropdownItem = {
		label: string;
		href: string;
		description?: string;
		icon?: Component;
		badge?: string;
	};

	export type NavFeatured = {
		title: string;
		description: string;
		href: string;
	};

	export type NavDropdown = {
		type: 'dropdown';
		label: string;
		featured?: NavFeatured;
		items: NavDropdownItem[];
	};

	export type NavLink = {
		type: 'link';
		label: string;
		href: string;
	};

	export type NavItem = NavLink | NavDropdown;
</script>

<script lang="ts">
	import * as NavigationMenu from '$lib/components/ui/navigation-menu/index.js';
	import { cn } from '$lib/utils.js';
	import { navigationMenuTriggerStyle } from '$lib/components/ui/navigation-menu/navigation-menu-trigger.svelte';
	import { IsMobile } from '$lib/components/hooks/is-mobile.svelte.js';

	const resolve = (href: string) => (href.startsWith('http') ? href : href);

	const isMobile = new IsMobile();

	let { items = [] }: { items: NavItem[] } = $props();
</script>

{#snippet DropdownItem(item: NavDropdownItem)}
	<li>
		<NavigationMenu.Link>
			{#snippet child()}
				<a
					href={resolve(item.href)}
					class={cn(
						'rounded-md p-3 leading-none no-underline transition-colors outline-none select-none hover:bg-foreground/10 focus:bg-foreground/10',
						item.icon ? 'flex items-center gap-2' : 'block',
						!item.icon && item.description && 'space-y-1'
					)}
					target={item.href.includes('http') ? '_blank' : undefined}
				>
					{#if item.icon}
						{@const Icon = item.icon}
						<Icon class="mr-4 size-5 shrink-0" strokeWidth={1.5} />
					{/if}
					<div>
						<div class="flex items-center gap-2 text-sm leading-none font-medium">
							{item.label}
							{#if item.badge}
								<span class="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
									{item.badge}
								</span>
							{/if}
						</div>
						{#if item.description}
							<p class="mt-1 line-clamp-2 text-sm leading-snug text-muted-foreground">
								{item.description}
							</p>
						{/if}
					</div>
				</a>
			{/snippet}
		</NavigationMenu.Link>
	</li>
{/snippet}

<div class="relative z-2">
	<NavigationMenu.Root viewport={isMobile.current}>
		<NavigationMenu.List class="flex-wrap">
			{#each items as item (item.label)}
				{#if item.type === 'link'}
					<NavigationMenu.Item>
						<NavigationMenu.Link>
							{#snippet child()}
								<a href={resolve(item.href)} class={navigationMenuTriggerStyle()}>{item.label}</a>
							{/snippet}
						</NavigationMenu.Link>
					</NavigationMenu.Item>
				{:else}
					<NavigationMenu.Item>
						<NavigationMenu.Trigger>{item.label}</NavigationMenu.Trigger>
						<NavigationMenu.Content>
							{#if item.featured}
								<ul class="grid gap-2 p-2 md:w-100 lg:w-125 lg:grid-cols-[.75fr_1fr]">
									<li style="grid-row: span {item.items.length} / span {item.items.length}">
										<NavigationMenu.Link
											class="flex h-full w-full flex-col justify-start rounded-md bg-linear-to-b from-muted/50 to-muted p-4 no-underline outline-hidden select-none focus:shadow-md"
										>
											{#snippet child({ props })}
												<a {...props} href={resolve(item.featured!.href)}>
													<div class="mb-2 text-lg font-medium">{item.featured!.title}</div>
													<p class="line-clamp-4 text-sm leading-snug text-muted-foreground">
														{item.featured!.description}
													</p>
												</a>
											{/snippet}
										</NavigationMenu.Link>
									</li>
									{#each item.items as di (di.href)}
										{@render DropdownItem(di)}
									{/each}
								</ul>
							{:else}
								<ul
									class={cn('grid gap-2 p-2', item.items.length > 4 ? 'w-125 grid-cols-2' : 'w-75')}
								>
									{#each item.items as di (di.href)}
										{@render DropdownItem(di)}
									{/each}
								</ul>
							{/if}
						</NavigationMenu.Content>
					</NavigationMenu.Item>
				{/if}
			{/each}
		</NavigationMenu.List>
	</NavigationMenu.Root>
</div>
