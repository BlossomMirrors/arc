<script lang="ts" module>
	import { tv, type VariantProps } from "tailwind-variants";

	export const buttonVariants = tv({
		base: "relative overflow-hidden inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-[var(--radius-button)] border border-transparent text-sm font-medium transition-colors duration-100 active:translate-y-[1px] disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive cursor-pointer",
		variants: {
			variant: {
				default:
					"bg-button text-button-foreground border-border hover:bg-button-hover",
				primary:
					"bg-button-accent/25 text-button-foreground border-primary/50 hover:bg-button-accent-hover/35 hover:border-primary/80",
				destructive:
					"bg-destructive text-destructive-foreground border-destructive hover:bg-destructive/85 hover:border-destructive/85 focus-visible:ring-destructive/30",
				ghost: "border-transparent text-foreground hover:bg-button-hover",
				link: "border-transparent text-link underline-offset-4 hover:underline",
			},
			size: {
				default: "h-9 px-4 py-2 has-[>svg]:px-3",
				sm: "h-8 gap-1.5 px-3 has-[>svg]:px-2.5",
				lg: "h-11 px-6 has-[>svg]:px-4",
				icon: "size-9",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	});
</script>

<script lang="ts">
	import { cn } from "$lib/utils.js";
	import type { HTMLButtonAttributes } from "svelte/elements";
	import type { Snippet } from "svelte";

	let {
		ref = $bindable(null),
		variant,
		size,
		child,
		children,
		class: className,
		onpointerdown,
		...restProps
	}: HTMLButtonAttributes &
		VariantProps<typeof buttonVariants> & {
			child?: Snippet<[{ props: Record<string, unknown> }]>;
			ref?: HTMLElement | null;
		} = $props();

	type Ripple = { id: number; x: number; y: number; size: number };
	let ripples = $state<Ripple[]>([]);
	let nextId = 0;

	function handlePointerDown(e: PointerEvent) {
		const target = e.currentTarget as HTMLElement;
		const rect = target.getBoundingClientRect();
		const x = e.clientX - rect.left;
		const y = e.clientY - rect.top;
		const size = Math.max(rect.width, rect.height) * 2;
		const id = nextId++;
		ripples = [...ripples, { id, x, y, size }];
		setTimeout(() => {
			ripples = ripples.filter((r) => r.id !== id);
		}, 600);
		if (typeof onpointerdown === "function") {
			onpointerdown(e as PointerEvent & { currentTarget: EventTarget & HTMLButtonElement });
		}
	}

	const mergedProps = $derived({
		...restProps,
		class: cn(buttonVariants({ variant, size }), className),
		"data-slot": "button",
	});

	const rippleColorClass = $derived(
		variant === "destructive" ? "bg-foreground" : "bg-primary"
	);
</script>

{#if child}
	{@render child({ props: mergedProps })}
{:else if children}
	<button {...mergedProps} bind:this={ref} onpointerdown={handlePointerDown}>
		{@render children?.()}
		{#each ripples as ripple (ripple.id)}
			<span
				class="blossom-ripple {rippleColorClass} pointer-events-none absolute rounded-full"
				style="left: {ripple.x}px; top: {ripple.y}px; width: {ripple.size}px; height: {ripple.size}px;"
			></span>
		{/each}
	</button>
{/if}

<style>
	.blossom-ripple {
		transform: translate(-50%, -50%) scale(0);
		opacity: 0.35;
		animation: blossom-ripple-expand 600ms ease-out forwards;
	}

	@keyframes blossom-ripple-expand {
		to {
			transform: translate(-50%, -50%) scale(1);
			opacity: 0;
		}
	}
</style>
