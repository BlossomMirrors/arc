<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { LogOut } from '@lucide/svelte';
	let { data, children } = $props();

	const user = $derived(data.user);

	function signOut() {
		window.location.href = '/auth/logout';
	}
</script>

<div class="flex min-h-screen flex-col">
	<header class="border-b border-border bg-background px-6 py-3">
		<div class="mx-auto flex w-full max-w-7xl items-center justify-between">
			<span class="text-sm font-medium text-muted-foreground">
				Welcome back, <span class="font-semibold text-foreground">{user.name.split(' ')[0]}</span>
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
						Sign out
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>
	</header>

	<main class="flex-1 px-6 py-8">
		<div class="mx-auto w-full max-w-7xl">
			{@render children()}
		</div>
	</main>
</div>
