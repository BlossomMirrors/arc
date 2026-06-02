<script lang="ts">
	import { enhance } from '$app/forms';
	import { tick } from 'svelte';
	import {
		Heading1, Heading2, Heading3, AlignLeft, List, Minus,
		Layers, Star, Sparkles, TrendingUp, LayoutGrid, Tag, LayoutList, BarChart2,
		Trash2, ChevronUp, ChevronDown, Plus
	} from '@lucide/svelte';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { newSection, type Section, type LangString, HTML_TYPES } from '$lib/frontpage.js';

	let { data } = $props();

	let sections = $state<Section[]>(structuredClone(data.sections) as Section[]);
	let dirty     = $state(false);
	let expandedIndex = $state<number | null>(null);

	// refs for focusing newly created text blocks
	let blockRefs: (HTMLTextAreaElement | HTMLInputElement | null)[] = [];

	// ── Palette state ────────────────────────────────────────────────────────
	let paletteOpen       = $state(false);
	let paletteFilter     = $state('');
	let paletteInsertAfter= $state(-1);
	let paletteX          = $state(0);
	let paletteY          = $state(0);
	let paletteInputEl    = $state<HTMLInputElement | null>(null);
	let selectedCmd       = $state(0);
	// when set, the palette replaces the block at this index instead of inserting
	let paletteReplaceAt  = $state<number | null>(null);

	function mark() { dirty = true; }

	function autoResize(el: HTMLTextAreaElement) {
		el.style.height = 'auto';
		el.style.height = el.scrollHeight + 'px';
	}

	// ── Commands ─────────────────────────────────────────────────────────────
	const COMMANDS = [
		// HTML / document
		{ type: 'h1'         as const, label: 'Heading 1',   icon: Heading1,    desc: 'Large heading'           },
		{ type: 'h2'         as const, label: 'Heading 2',   icon: Heading2,    desc: 'Medium heading'          },
		{ type: 'h3'         as const, label: 'Heading 3',   icon: Heading3,    desc: 'Small heading'           },
		{ type: 'p'          as const, label: 'Paragraph',   icon: AlignLeft,   desc: 'Body text'               },
		{ type: 'ul'         as const, label: 'Bullet list', icon: List,        desc: 'Unordered list'          },
		{ type: 'br'         as const, label: 'Divider',     icon: Minus,       desc: 'Horizontal break'        },
		// App-store sections
		{ type: 'carousel'   as const, label: 'Carousel',    icon: Layers,      desc: 'Featured app slideshow'  },
		{ type: 'top'        as const, label: 'Top Apps',    icon: Star,        desc: 'Highest-rated apps'       },
		{ type: 'new'        as const, label: 'New',         icon: Sparkles,    desc: 'Recently added apps'      },
		{ type: 'trending'   as const, label: 'Trending',    icon: TrendingUp,  desc: 'Trending apps'           },
		{ type: 'categories' as const, label: 'Categories',  icon: LayoutGrid,  desc: 'Category grid'           },
		{ type: 'category'   as const, label: 'Category',    icon: Tag,         desc: 'Single category row'     },
		{ type: 'custom'     as const, label: 'Custom',      icon: LayoutList,  desc: 'Curated list with title' },
		{ type: 'charts'     as const, label: 'Charts',      icon: BarChart2,   desc: 'App rankings'            },
	];

	const filtered = $derived(
		paletteFilter
			? COMMANDS.filter(c =>
				c.label.toLowerCase().includes(paletteFilter.toLowerCase()) ||
				c.desc.toLowerCase().includes(paletteFilter.toLowerCase()))
			: COMMANDS
	);

	async function openPalette(afterIndex: number, x: number, y: number, replaceAt: number | null = null) {
		paletteInsertAfter = afterIndex;
		paletteReplaceAt   = replaceAt;
		paletteFilter      = '';
		selectedCmd        = 0;
		paletteX           = Math.min(x, window.innerWidth - 290);
		paletteY           = y + 6;
		paletteOpen        = true;
		await tick();
		paletteInputEl?.focus();
	}

	function closePalette() { paletteOpen = false; paletteFilter = ''; paletteReplaceAt = null; }

	async function pick(type: Section['type']) {
		const arr = [...sections];
		let focusIdx: number;
		if (paletteReplaceAt !== null) {
			arr[paletteReplaceAt] = newSection(type);
			focusIdx = paletteReplaceAt;
		} else {
			arr.splice(paletteInsertAfter + 1, 0, newSection(type));
			focusIdx = paletteInsertAfter + 1;
		}
		sections = arr;
		expandedIndex = HTML_TYPES.includes(type as typeof HTML_TYPES[number]) ? null : focusIdx;
		closePalette();
		mark();
		await tick();
		blockRefs[focusIdx]?.focus();
	}

	function paletteKeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') { e.preventDefault(); selectedCmd = Math.min(selectedCmd + 1, filtered.length - 1); }
		if (e.key === 'ArrowUp')   { e.preventDefault(); selectedCmd = Math.max(selectedCmd - 1, 0); }
		if (e.key === 'Enter')     { e.preventDefault(); if (filtered[selectedCmd]) pick(filtered[selectedCmd].type); }
		if (e.key === 'Escape')    closePalette();
	}

	// ── Block operations ──────────────────────────────────────────────────────
	function remove(i: number) {
		sections = sections.filter((_, j) => j !== i);
		if (expandedIndex === i) expandedIndex = null;
		mark();
	}
	function moveUp(i: number) {
		if (i === 0) return;
		const s = [...sections]; [s[i-1], s[i]] = [s[i], s[i-1]]; sections = s;
		if (expandedIndex === i) expandedIndex = i - 1;
		mark();
	}
	function moveDown(i: number) {
		if (i === sections.length - 1) return;
		const s = [...sections]; [s[i], s[i+1]] = [s[i+1], s[i]]; sections = s;
		if (expandedIndex === i) expandedIndex = i + 1;
		mark();
	}

	// ── Text block keyboard handling ──────────────────────────────────────────
	async function textKeydown(e: KeyboardEvent, i: number, section: Extract<Section, { text: string }>) {
		// "/" on empty block → open palette to replace this block
		if (e.key === '/' && section.text === '') {
			e.preventDefault();
			const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
			openPalette(i - 1, rect.left, rect.bottom, i);
			return;
		}
		// Enter → insert new paragraph after
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			const arr = [...sections];
			arr.splice(i + 1, 0, { type: 'p', text: '' });
			sections = arr;
			mark();
			await tick();
			blockRefs[i + 1]?.focus();
		}
		// Backspace on empty block → remove
		if (e.key === 'Backspace' && section.text === '') {
			e.preventDefault();
			remove(i);
			await tick();
			if (i > 0) blockRefs[i - 1]?.focus();
		}
	}

	async function listItemKeydown(e: KeyboardEvent, section: Extract<Section, { type: 'ul' }>, itemIdx: number) {
		if (e.key === 'Enter') {
			e.preventDefault();
			section.items.splice(itemIdx + 1, 0, '');
			section.items = [...section.items];
			mark();
			await tick();
			const listRefs = document.querySelectorAll<HTMLInputElement>(`[data-list-item]`);
			listRefs[itemIdx + 1]?.focus();
		}
		if (e.key === 'Backspace' && section.items[itemIdx] === '') {
			e.preventDefault();
			if (section.items.length === 1) return;
			section.items.splice(itemIdx, 1);
			section.items = [...section.items];
			mark();
		}
	}

	// ── Helpers ───────────────────────────────────────────────────────────────
	type Carousel = Extract<Section, { type: 'carousel' }>;
	type Custom   = Extract<Section, { type: 'custom' }>;

	function addCarouselApp  (s: Carousel) { s.items = [...s.items, { type: 'app', id: '' }]; mark(); }
	function addCarouselStory(s: Carousel) { s.items = [...s.items, { type: 'story', banner: '', titles: [{ lang: 'en', text: '' }], body: '' }]; mark(); }
	function removeCarouselItem(s: Carousel, j: number) { s.items = s.items.filter((_, k) => k !== j); mark(); }
	function addCustomApp    (s: Custom)  { s.apps = [...s.apps, '']; mark(); }
	function removeCustomApp (s: Custom, j: number) { s.apps = s.apps.filter((_, k) => k !== j); mark(); }
	function addTitle  (arr: LangString[]) { arr.push({ lang: 'de', text: '' }); mark(); }
	function removeTitle(arr: LangString[], j: number) { arr.splice(j, 1); mark(); }

	// ── App-section label ─────────────────────────────────────────────────────
	const LABEL: Record<string, string> = {
		carousel: 'Carousel', top: 'Top Apps', new: 'New', trending: 'Trending',
		categories: 'Categories', category: 'Category', custom: 'Custom', charts: 'Charts',
	};
</script>

<svelte:head><title>Front Page Designer — Arc Forge</title></svelte:head>

<form method="POST" action="?/save" use:enhance={() => async ({ update }) => { await update(); dirty = false; }}>
	<input type="hidden" name="sections" value={JSON.stringify(sections)} />

	<div class="mb-6 flex items-center justify-between">
		<div>
			<h2 class="text-lg font-semibold">Front Page</h2>
			<p class="text-xs text-muted-foreground">
				<code class="font-mono">/api/frontpage</code> ·
				press <kbd class="rounded border border-border bg-muted px-1 font-mono text-[10px]">/</kbd> to insert a block
			</p>
		</div>
		<Button type="submit" disabled={!dirty} variant={dirty ? 'default' : 'ghost'}>
			{dirty ? 'Save' : 'Saved'}
		</Button>
	</div>

	<!-- Document canvas -->
	<div class="mx-auto max-w-2xl py-4">
		{#if sections.length === 0}
			<p
				class="cursor-text select-none text-muted-foreground/30"
				role="button"
				tabindex="0"
				onclick={(e) => openPalette(-1, e.clientX, e.clientY)}
				onkeydown={(e) => e.key === '/' && openPalette(-1, 80, 140)}
			>
				Press <span class="font-mono">/</span> to start writing…
			</p>
		{:else}
			{#each sections as section, i (i)}
				{@const isApp = section.type in LABEL}

				<div class="group relative">
					<!-- ── Hover side controls ───────────────────────────────── -->
					<div class="absolute -left-14 top-1 hidden items-center gap-0.5 group-hover:flex">
						<button type="button" onclick={() => { const r = document.querySelectorAll('[data-block]')[i]?.getBoundingClientRect(); openPalette(i - 1, (r?.left ?? 80) - 10, (r?.bottom ?? 80), null); }}
							class="rounded p-1 text-muted-foreground/30 hover:bg-muted hover:text-muted-foreground">
							<Plus class="size-3.5" />
						</button>
						<button type="button" onclick={() => moveUp(i)} disabled={i === 0} class="rounded p-1 text-muted-foreground/30 hover:text-muted-foreground disabled:pointer-events-none">
							<ChevronUp class="size-3.5" />
						</button>
						<button type="button" onclick={() => moveDown(i)} disabled={i === sections.length - 1} class="rounded p-1 text-muted-foreground/30 hover:text-muted-foreground disabled:pointer-events-none">
							<ChevronDown class="size-3.5" />
						</button>
						<button type="button" onclick={() => remove(i)} class="rounded p-1 text-muted-foreground/30 hover:text-destructive">
							<Trash2 class="size-3.5" />
						</button>
					</div>

					<!-- ── Block content ─────────────────────────────────────── -->
					<div data-block>
						{#if section.type === 'h1'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-3xl font-bold tracking-tight outline-none placeholder:text-muted-foreground/30"
								placeholder="Heading 1"
								bind:value={section.text}
								oninput={(e) => { autoResize(e.currentTarget); mark(); }}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>

						{:else if section.type === 'h2'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-2xl font-semibold tracking-tight outline-none placeholder:text-muted-foreground/30"
								placeholder="Heading 2"
								bind:value={section.text}
								oninput={(e) => { autoResize(e.currentTarget); mark(); }}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>

						{:else if section.type === 'h3'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-xl font-semibold outline-none placeholder:text-muted-foreground/30"
								placeholder="Heading 3"
								bind:value={section.text}
								oninput={(e) => { autoResize(e.currentTarget); mark(); }}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>

						{:else if section.type === 'p'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-base leading-7 text-foreground outline-none placeholder:text-muted-foreground/30"
								placeholder="Start typing, or press / for commands…"
								bind:value={section.text}
								oninput={(e) => { autoResize(e.currentTarget); mark(); }}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>

						{:else if section.type === 'ul'}
							<ul class="my-1 space-y-0.5 pl-5">
								{#each section.items as _, j (j)}
									<li class="flex items-start gap-2">
										<span class="mt-2 size-1.5 shrink-0 rounded-full bg-foreground/40"></span>
										<input
											data-list-item
											class="flex-1 bg-transparent text-base leading-7 outline-none placeholder:text-muted-foreground/30"
											placeholder="List item"
											bind:value={section.items[j]}
											oninput={mark}
											onkeydown={(e) => listItemKeydown(e, section, j)}
										/>
									</li>
								{/each}
							</ul>

						{:else if section.type === 'br'}
							<div class="my-4 flex items-center gap-3 text-muted-foreground/30">
								<hr class="flex-1 border-border/40" />
							</div>

						{:else if isApp}
							<!-- App-store section block -->
							{@const expanded = expandedIndex === i}
							<div class="my-1 rounded-lg border border-border/40 hover:border-border/70 {expanded ? 'border-border' : ''}">
								<div
									class="flex cursor-pointer items-center gap-3 px-4 py-2.5 text-sm"
									role="button"
									tabindex="0"
									onclick={() => expandedIndex = expanded ? null : i}
									onkeydown={(e) => e.key === 'Enter' && (expandedIndex = expanded ? null : i)}
								>
									<span class="font-medium text-muted-foreground">{LABEL[section.type]}</span>
									<span class="text-xs text-muted-foreground/50">
										{#if section.type === 'carousel'}{section.items.length} items · bp={section.breakpoint}
										{:else if section.type === 'category'}{section.value || '—'}
										{:else if section.type === 'custom'}{section.titles[0]?.text || '—'}
										{:else if section.type === 'charts'}cards={section.cards}
										{:else}–{/if}
									</span>
								</div>

								{#if expanded}
									<div class="border-t border-border/40 px-4 py-3 text-sm">
										{#if section.type === 'carousel'}
											<div class="flex gap-4 mb-3">
												<label class="space-y-1">
													<span class="text-xs text-muted-foreground">Breakpoint</span>
													<Input type="number" class="h-8 w-24 text-sm" bind:value={section.breakpoint} oninput={mark} min={1} />
												</label>
												<label class="flex items-center gap-2 self-end pb-0.5 text-sm">
													<input type="checkbox" bind:checked={section.flathub} onchange={mark} /> Flathub
												</label>
											</div>
											{#each section.items as item, j (j)}
												<div class="mb-2 space-y-2 rounded border border-border p-3">
													<div class="flex items-center justify-between">
														<span class="text-xs uppercase tracking-wide text-muted-foreground">{item.type}</span>
														<button type="button" onclick={() => removeCarouselItem(section, j)} class="text-muted-foreground hover:text-destructive"><Trash2 class="size-3.5" /></button>
													</div>
													{#if item.type === 'app'}
														<Input placeholder="com.example.App" bind:value={item.id} oninput={mark} class="h-8 font-mono text-sm" />
													{:else}
														<Input placeholder="banner.jpg" bind:value={item.banner} oninput={mark} class="h-8 text-sm" />
														{#each item.titles as t, k (k)}
															<div class="flex gap-2">
																<Input class="h-8 w-14 text-sm" placeholder="en" bind:value={t.lang} oninput={mark} />
																<Input class="h-8 flex-1 text-sm" placeholder="Title" bind:value={t.text} oninput={mark} />
																<button type="button" onclick={() => removeTitle(item.titles, k)} class="text-muted-foreground hover:text-destructive"><Trash2 class="size-3.5" /></button>
															</div>
														{/each}
														<button type="button" class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" onclick={() => addTitle(item.titles)}><Plus class="size-3" /> title</button>
														<textarea class="mt-1 w-full rounded border border-input bg-muted/30 px-3 py-2 font-mono text-xs outline-none" rows={3} placeholder="Body XML…" bind:value={item.body} oninput={mark}></textarea>
													{/if}
												</div>
											{/each}
											<div class="flex gap-3">
												<button type="button" class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" onclick={() => addCarouselApp(section)}><Plus class="size-3" /> app</button>
												<button type="button" class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" onclick={() => addCarouselStory(section)}><Plus class="size-3" /> story</button>
											</div>

										{:else if section.type === 'category'}
											<Input placeholder="games" bind:value={section.value} oninput={mark} class="h-8 max-w-xs text-sm" />

										{:else if section.type === 'charts'}
											<label class="flex items-center gap-2 text-sm">
												<input type="checkbox" bind:checked={section.cards} onchange={mark} /> Cards view
											</label>

										{:else if section.type === 'custom'}
											<div class="space-y-3">
												<div class="space-y-2">
													<p class="text-xs text-muted-foreground">Titles</p>
													{#each section.titles as t, k (k)}
														<div class="flex gap-2">
															<Input class="h-8 w-14 text-sm" placeholder="en" bind:value={t.lang} oninput={mark} />
															<Input class="h-8 flex-1 text-sm" placeholder="Title" bind:value={t.text} oninput={mark} />
															<button type="button" onclick={() => removeTitle(section.titles, k)} class="text-muted-foreground hover:text-destructive"><Trash2 class="size-3.5" /></button>
														</div>
													{/each}
													<button type="button" class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" onclick={() => addTitle(section.titles)}><Plus class="size-3" /> title</button>
												</div>
												<div class="space-y-2">
													<p class="text-xs text-muted-foreground">Apps</p>
													{#each section.apps as _, k (k)}
														<div class="flex gap-2">
															<Input class="h-8 flex-1 font-mono text-sm" placeholder="io.github.example.App" bind:value={section.apps[k]} oninput={mark} />
															<button type="button" onclick={() => removeCustomApp(section, k)} class="text-muted-foreground hover:text-destructive"><Trash2 class="size-3.5" /></button>
														</div>
													{/each}
													<button type="button" class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground" onclick={() => addCustomApp(section)}><Plus class="size-3" /> app</button>
												</div>
											</div>

										{:else}
											<p class="text-xs italic text-muted-foreground">No configuration.</p>
										{/if}
									</div>
								{/if}
							</div>
						{/if}
					</div>
				</div>
			{/each}

			<!-- Trailing slash hint -->
			<div
				class="mt-1 cursor-text py-2 text-sm text-muted-foreground/20 transition-colors hover:text-muted-foreground/40"
				role="button"
				tabindex="0"
				onclick={(e) => openPalette(sections.length - 1, e.clientX, e.clientY)}
				onkeydown={(e) => { if (e.key === '/') { e.preventDefault(); openPalette(sections.length - 1, 80, e.currentTarget.getBoundingClientRect().top); } }}
			>
				<span class="font-mono">/</span> Type to add a block…
			</div>
		{/if}
	</div>
</form>

<!-- ── Slash command palette ──────────────────────────────────────────────── -->
{#if paletteOpen}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 z-40" onclick={closePalette} onkeydown={(e) => e.key === 'Escape' && closePalette()}></div>
	<div class="fixed z-50 w-72 overflow-hidden rounded-xl border border-border bg-background shadow-2xl" style="top:{paletteY}px;left:{paletteX}px">
		<div class="flex items-center gap-2 border-b border-border px-3 py-2">
			<span class="font-mono text-xs text-muted-foreground">/</span>
			<input
				bind:this={paletteInputEl}
				bind:value={paletteFilter}
				onkeydown={paletteKeydown}
				oninput={() => selectedCmd = 0}
				placeholder="Search blocks…"
				class="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/40"
			/>
		</div>
		<ul class="max-h-80 overflow-y-auto py-1">
			{#each filtered as cmd, i (cmd.type)}
				<li>
					<button
						type="button"
						class="flex w-full items-center gap-3 px-3 py-2 text-left text-sm {selectedCmd === i ? 'bg-muted' : 'hover:bg-muted'}"
						onclick={() => pick(cmd.type)}
						onmouseenter={() => selectedCmd = i}
					>
						<div class="flex size-7 shrink-0 items-center justify-center rounded-md border border-border bg-muted/50">
							<cmd.icon class="size-3.5 text-muted-foreground" />
						</div>
						<div class="min-w-0">
							<p class="font-medium">{cmd.label}</p>
							<p class="truncate text-xs text-muted-foreground">{cmd.desc}</p>
						</div>
					</button>
				</li>
			{/each}
			{#if filtered.length === 0}
				<li class="px-3 py-4 text-center text-xs text-muted-foreground">No results</li>
			{/if}
		</ul>
	</div>
{/if}
