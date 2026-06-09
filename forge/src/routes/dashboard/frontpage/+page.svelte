<script lang="ts">
	import { enhance } from '$app/forms';
	import { tick, untrack } from 'svelte';
	import {
		Heading1,
		Heading2,
		Heading3,
		AlignLeft,
		List,
		Minus,
		Layers,
		Star,
		Sparkles,
		TrendingUp,
		LayoutGrid,
		Tag,
		LayoutList,
		BarChart2,
		Trash2,
		ChevronUp,
		ChevronDown,
		Plus,
		GripVertical,
		Link
	} from '@lucide/svelte';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { newSection, type Section, type LangString, type LinksItem, HTML_TYPES } from '$lib/frontpage.js';
	import * as m from '$lib/paraglide/messages';
	import UploadButton from '$lib/components/upload-button.svelte';

	let { data } = $props();

	let sections = $state<Section[]>(untrack(() => structuredClone(data.sections) as Section[]));
	let dirty = $state(false);
	let expandedIndex = $state<number | null>(null);
	let blockRefs: (HTMLTextAreaElement | HTMLInputElement | null)[] = [];

	// Palette state
	let paletteOpen = $state(false);
	let paletteFilter = $state('');
	let selectedCmd = $state(0);
	let paletteX = $state(0);
	let paletteY = $state(0);
	// Which block's text triggered the slash, and where the '/' is in that text
	let slashBlockIdx = $state<number | null>(null);
	let slashStart = $state(0);

	// Drag state
	let draggingIdx = $state<number | null>(null);
	let dragOverIdx = $state<number | null>(null);

	function mark() {
		dirty = true;
	}

	function autoResize(el: HTMLTextAreaElement) {
		el.style.height = 'auto';
		el.style.height = el.scrollHeight + 'px';
	}

	const COMMANDS = $derived([
		{ type: 'h1' as const, label: m.cmd_heading1(), badge: 'H₁', icon: Heading1, shorthand: '#' },
		{ type: 'h2' as const, label: m.cmd_heading2(), badge: 'H₂', icon: Heading2, shorthand: '##' },
		{ type: 'h3' as const, label: m.cmd_heading3(), badge: 'H₃', icon: Heading3, shorthand: '###' },
		{ type: 'p' as const, label: m.cmd_paragraph(), badge: 'P', icon: AlignLeft, shorthand: '' },
		{ type: 'ul' as const, label: m.cmd_bullet(), badge: null, icon: List, shorthand: '-' },
		{ type: 'br' as const, label: m.cmd_divider(), badge: null, icon: Minus, shorthand: '---' },
		{
			type: 'carousel' as const,
			label: m.cmd_carousel(),
			badge: null,
			icon: Layers,
			shorthand: ''
		},
		{ type: 'top' as const, label: m.cmd_top(), badge: null, icon: Star, shorthand: '' },
		{ type: 'new' as const, label: m.cmd_new(), badge: null, icon: Sparkles, shorthand: '' },
		{
			type: 'trending' as const,
			label: m.cmd_trending(),
			badge: null,
			icon: TrendingUp,
			shorthand: ''
		},
		{
			type: 'categories' as const,
			label: m.cmd_categories(),
			badge: null,
			icon: LayoutGrid,
			shorthand: ''
		},
		{ type: 'category' as const, label: m.cmd_category(), badge: null, icon: Tag, shorthand: '' },
		{
			type: 'custom' as const,
			label: m.cmd_custom(),
			badge: null,
			icon: LayoutList,
			shorthand: ''
		},
		{ type: 'charts' as const, label: m.cmd_charts(), badge: null, icon: BarChart2, shorthand: '' },
		{ type: 'links' as const, label: m.cmd_links(), badge: null, icon: Link, shorthand: '' }
	]);

	const filtered = $derived(
		paletteFilter
			? COMMANDS.filter((c) => c.label.toLowerCase().includes(paletteFilter.toLowerCase()))
			: COMMANDS
	);

	function closePalette() {
		paletteOpen = false;
		paletteFilter = '';
		slashBlockIdx = null;
	}

	async function pick(type: Section['type'] | undefined) {
		if (!type || slashBlockIdx === null) return;
		const arr = [...sections];
		const sec = arr[slashBlockIdx];
		let focusIdx = slashBlockIdx;

		if ('text' in sec) {
			const textBefore = sec.text.slice(0, slashStart);
			const isHtml = HTML_TYPES.includes(type as (typeof HTML_TYPES)[number]);
			if (isHtml) {
				const ns = newSection(type);
				if ('text' in ns) (ns as { text: string }).text = textBefore;
				arr[slashBlockIdx] = ns;
			} else if (textBefore.trim() === '') {
				arr[slashBlockIdx] = newSection(type);
			} else {
				(sec as { text: string }).text = textBefore;
				arr.splice(slashBlockIdx + 1, 0, newSection(type));
				focusIdx = slashBlockIdx + 1;
			}
		}

		sections = arr;
		expandedIndex = HTML_TYPES.includes(type as (typeof HTML_TYPES)[number]) ? null : focusIdx;
		closePalette();
		mark();
		await tick();
		blockRefs[focusIdx]?.focus();
	}

	// Open palette anchored to a textarea, triggered by inserting a '/' via addViaSlash
	async function openForBlock(blockIdx: number, el: HTMLTextAreaElement | HTMLInputElement) {
		const rect = el.getBoundingClientRect();
		slashBlockIdx = blockIdx;
		slashStart = 0;
		paletteFilter = '';
		selectedCmd = 0;
		paletteX = Math.min(rect.left, window.innerWidth - 290);
		paletteY = rect.bottom + 6;
		paletteOpen = true;
	}

	// Insert a '/' paragraph and open the palette used by + button and global /
	async function addViaSlash(afterIdx: number) {
		const arr = [...sections];
		arr.splice(afterIdx + 1, 0, { type: 'p', text: '/' });
		sections = arr;
		mark();
		await tick();
		const newIdx = afterIdx + 1;
		const el = blockRefs[newIdx];
		if (el) {
			el.focus();
			openForBlock(newIdx, el);
		}
	}

	async function textKeydown(
		e: KeyboardEvent,
		i: number,
		section: Extract<Section, { text: string }>
	) {
		// Palette navigation while it's open for this block
		if (paletteOpen && slashBlockIdx === i) {
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				selectedCmd = Math.min(selectedCmd + 1, filtered.length - 1);
				return;
			}
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				selectedCmd = Math.max(selectedCmd - 1, 0);
				return;
			}
			if (e.key === 'Enter') {
				e.preventDefault();
				pick(filtered[selectedCmd]?.type);
				return;
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				closePalette();
				return;
			}
			// All other keys: let browser handle; oninput updates filter
			return;
		}

		// Normal block editing
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			const arr = [...sections];
			arr.splice(i + 1, 0, { type: 'p', text: '' });
			sections = arr;
			mark();
			await tick();
			blockRefs[i + 1]?.focus();
		}
		if (e.key === 'Backspace' && section.text === '') {
			e.preventDefault();
			remove(i);
			await tick();
			if (i > 0) blockRefs[i - 1]?.focus();
		}
	}

	// Called on every oninput for text blocks
	// Handles both slash detection and filter updates
	function handleTextInput(e: Event, i: number, section: Extract<Section, { text: string }>) {
		const ta = e.currentTarget as HTMLTextAreaElement;
		autoResize(ta);
		mark();

		const ie = e as InputEvent;

		// Detect '/' typed (not pasted)
		if (!paletteOpen && ie.inputType === 'insertText' && ie.data === '/') {
			const pos = ta.selectionStart ?? 0;
			slashBlockIdx = i;
			slashStart = pos - 1; // '/' just landed at pos-1
			const rect = ta.getBoundingClientRect();
			paletteFilter = '';
			selectedCmd = 0;
			paletteX = Math.min(rect.left, window.innerWidth - 290);
			paletteY = rect.bottom + 6;
			paletteOpen = true;
			return;
		}

		// Update filter while palette is open
		if (paletteOpen && slashBlockIdx === i) {
			const cursorPos = ta.selectionStart ?? 0;
			// Slash was deleted or cursor moved before it
			if (cursorPos <= slashStart || section.text[slashStart] !== '/') {
				closePalette();
				return;
			}
			const filter = section.text.slice(slashStart + 1);
			// Space after slash closes palette
			if (filter.includes(' ')) {
				closePalette();
				return;
			}
			paletteFilter = filter;
			selectedCmd = 0;
		}
	}

	function remove(i: number) {
		sections = sections.filter((_s, j) => j !== i);
		if (expandedIndex === i) expandedIndex = null;
		mark();
	}
	function moveUp(i: number) {
		if (i === 0) return;
		const s = [...sections];
		[s[i - 1], s[i]] = [s[i], s[i - 1]];
		sections = s;
		if (expandedIndex === i) expandedIndex = i - 1;
		mark();
	}
	function moveDown(i: number) {
		if (i === sections.length - 1) return;
		const s = [...sections];
		[s[i], s[i + 1]] = [s[i + 1], s[i]];
		sections = s;
		if (expandedIndex === i) expandedIndex = i + 1;
		mark();
	}

	function startDrag(i: number, e: DragEvent) {
		draggingIdx = i;
		e.dataTransfer?.setData('text/plain', String(i));
	}
	function onDragOver(e: DragEvent, i: number) {
		e.preventDefault();
		dragOverIdx = i;
	}
	function onDrop(i: number) {
		if (draggingIdx === null || draggingIdx === i) {
			draggingIdx = null;
			dragOverIdx = null;
			return;
		}
		const arr = [...sections];
		const [moved] = arr.splice(draggingIdx, 1);
		arr.splice(draggingIdx < i ? i - 1 : i, 0, moved);
		sections = arr;
		mark();
		draggingIdx = null;
		dragOverIdx = null;
	}
	function onDragEnd() {
		draggingIdx = null;
		dragOverIdx = null;
	}

	async function listItemKeydown(
		e: KeyboardEvent,
		section: Extract<Section, { type: 'ul' }>,
		itemIdx: number
	) {
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

	type Carousel = Extract<Section, { type: 'carousel' }>;
	type Custom = Extract<Section, { type: 'custom' }>;
	type Links = Extract<Section, { type: 'links' }>;

	function addCarouselApp(s: Carousel) {
		s.items = [...s.items, { type: 'app', id: '' }];
		mark();
	}
	function addCarouselStory(s: Carousel) {
		s.items = [
			...s.items,
			{ type: 'story', banner: '', titles: [{ lang: 'en', text: '' }], body: '' }
		];
		mark();
	}
	function removeCarouselItem(s: Carousel, j: number) {
		s.items = s.items.filter((_s, k) => k !== j);
		mark();
	}
	function addCustomApp(s: Custom) {
		s.apps = [...s.apps, ''];
		mark();
	}
	function removeCustomApp(s: Custom, j: number) {
		s.apps = s.apps.filter((_s, k) => k !== j);
		mark();
	}
	function addTitle(arr: LangString[]) {
		arr.push({ lang: 'de', text: '' });
		mark();
	}
	function removeTitle(arr: LangString[], j: number) {
		arr.splice(j, 1);
		mark();
	}
	function addLinksUrl(s: Links) {
		s.items = [...s.items, { kind: 'url', text: '', href: '' }];
		mark();
	}
	function addLinksApp(s: Links) {
		s.items = [...s.items, { kind: 'app', id: '' }];
		mark();
	}
	function addLinksStory(s: Links) {
		s.items = [...s.items, { kind: 'story', banner: '', titles: [{ lang: 'en', text: '' }], body: '' }];
		mark();
	}
	function removeLinksItem(s: Links, j: number) {
		s.items = s.items.filter((_: LinksItem, k: number) => k !== j);
		mark();
	}

	async function insertParagraph(afterIndex: number) {
		const arr = [...sections];
		arr.splice(afterIndex + 1, 0, { type: 'p', text: '' });
		sections = arr;
		mark();
		await tick();
		blockRefs[afterIndex + 1]?.focus();
	}

	let formEl = $state<HTMLFormElement | null>(null);

	async function handleGlobalKeydown(e: KeyboardEvent) {
		if (e.code === 'KeyS' && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
			e.preventDefault();
			if (dirty) formEl?.requestSubmit();
			return;
		}
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
		if (paletteOpen) {
			if (e.key === 'Escape') closePalette();
			return;
		}
		if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && !e.altKey) {
			e.preventDefault();
			insertParagraph(sections.length - 1);
			return;
		}
		if (e.key === '/' && !e.ctrlKey && !e.metaKey && !e.altKey) {
			e.preventDefault();
			addViaSlash(sections.length - 1);
		}
	}

	const APP_TYPES = new Set([
		'carousel',
		'top',
		'new',
		'trending',
		'categories',
		'category',
		'custom',
		'charts',
		'links'
	]);

	const SECTION_LABELS: Record<string, () => string> = {
		carousel: m.cmd_carousel,
		top: m.cmd_top,
		new: m.cmd_new,
		trending: m.cmd_trending,
		categories: m.cmd_categories,
		category: m.cmd_category,
		custom: m.cmd_custom,
		charts: m.cmd_charts,
		links: m.cmd_links
	};
</script>

<svelte:head><title>Front Page Designer - Arc Forge</title></svelte:head>
<svelte:window onkeydown={handleGlobalKeydown} />

<form
	bind:this={formEl}
	method="POST"
	action="?/save"
	use:enhance={() =>
		async ({ result }) => {
			if (result.type === 'success') dirty = false;
		}}
>
	<input type="hidden" name="sections" value={JSON.stringify($state.snapshot(sections))} />

	<div class="mb-6 flex items-center justify-between">
		<div>
			<h2 class="text-lg font-semibold">{m.frontpage_heading()}</h2>
			<p class="text-xs text-muted-foreground">
				<code class="font-mono">/api/frontpage</code> · {m.frontpage_hint()}
			</p>
		</div>
		<Button type="submit" disabled={!dirty} variant={dirty ? 'default' : 'ghost'}>
			{dirty ? m.frontpage_save() : m.frontpage_saved()}
		</Button>
	</div>

	<div class="mx-auto max-w-2xl py-4">
		{#if sections.length === 0}
			<div
				class="cursor-text text-muted-foreground/30 select-none"
				role="button"
				tabindex="0"
				onclick={() => insertParagraph(-1)}
				onkeydown={(e) => {
					if (e.key === 'Enter') insertParagraph(-1);
				}}
			>
				{m.frontpage_click_or_press()} <span class="font-mono">Enter</span>
				{m.frontpage_to_write()}
				<span class="font-mono">/</span>
				{m.frontpage_for_commands()}
			</div>
		{:else}
			{#each sections as section, i (i)}
				{@const isApp = APP_TYPES.has(section.type)}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="group relative pl-14 {draggingIdx === i ? 'opacity-40' : ''} {dragOverIdx === i &&
					draggingIdx !== null &&
					draggingIdx !== i
						? 'border-t-2 border-primary'
						: ''}"
					ondragover={(e) => onDragOver(e, i)}
					ondrop={() => onDrop(i)}
					ondragleave={(e) => {
						if (!e.currentTarget.contains(e.relatedTarget as Node)) dragOverIdx = null;
					}}
				>
					<!-- Left: + and drag handle -->
					<div class="absolute top-0.5 left-0 hidden items-center gap-0.5 group-hover:flex">
						<button
							type="button"
							onclick={(e) => {
								e.stopPropagation();
								addViaSlash(i);
							}}
							class="rounded p-1 text-muted-foreground/30 hover:bg-muted hover:text-muted-foreground"
							title="Add block"
						>
							<Plus class="size-3.5" />
						</button>
						<button
							type="button"
							draggable="true"
							ondragstart={(e) => startDrag(i, e)}
							ondragend={onDragEnd}
							class="cursor-grab rounded p-1 text-muted-foreground/30 hover:bg-muted hover:text-muted-foreground active:cursor-grabbing"
							title="Drag to reorder"
						>
							<GripVertical class="size-3.5" />
						</button>
					</div>

					<!-- Right: move + delete -->
					<div
						class="absolute top-0 right-0 z-10 hidden items-center gap-0.5 rounded-md border border-border bg-background px-0.5 py-0.5 shadow-sm group-hover:flex"
					>
						<button
							type="button"
							onclick={(e) => {
								e.stopPropagation();
								moveUp(i);
							}}
							disabled={i === 0}
							class="rounded p-1 text-muted-foreground/50 hover:text-muted-foreground disabled:pointer-events-none disabled:opacity-30"
							title="Move up"><ChevronUp class="size-3.5" /></button
						>
						<button
							type="button"
							onclick={(e) => {
								e.stopPropagation();
								moveDown(i);
							}}
							disabled={i === sections.length - 1}
							class="rounded p-1 text-muted-foreground/50 hover:text-muted-foreground disabled:pointer-events-none disabled:opacity-30"
							title="Move down"><ChevronDown class="size-3.5" /></button
						>
						<button
							type="button"
							onclick={(e) => {
								e.stopPropagation();
								remove(i);
							}}
							class="rounded p-1 text-muted-foreground/50 hover:text-destructive"
							title="Delete"><Trash2 class="size-3.5" /></button
						>
					</div>

					<div data-block>
						{#if section.type === 'h1'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-3xl font-bold tracking-tight outline-none placeholder:text-muted-foreground/30"
								placeholder="Heading 1"
								bind:value={section.text}
								oninput={(e) => handleTextInput(e, i, section)}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>
						{:else if section.type === 'h2'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-2xl font-semibold tracking-tight outline-none placeholder:text-muted-foreground/30"
								placeholder="Heading 2"
								bind:value={section.text}
								oninput={(e) => handleTextInput(e, i, section)}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>
						{:else if section.type === 'h3'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-xl font-semibold outline-none placeholder:text-muted-foreground/30"
								placeholder="Heading 3"
								bind:value={section.text}
								oninput={(e) => handleTextInput(e, i, section)}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>
						{:else if section.type === 'p'}
							<textarea
								bind:this={blockRefs[i] as HTMLTextAreaElement}
								rows={1}
								class="block w-full resize-none bg-transparent text-base leading-7 text-foreground outline-none placeholder:text-muted-foreground/30"
								bind:value={section.text}
								oninput={(e) => handleTextInput(e, i, section)}
								onkeydown={(e) => textKeydown(e, i, section)}
							></textarea>
						{:else if section.type === 'ul'}
							<ul class="my-1 space-y-0.5 pl-5">
								{#each section.items, j (j)}
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
							{@const expanded = expandedIndex === i}
							<div
								class="my-1 rounded-lg border border-border/40 hover:border-border/70 {expanded
									? 'border-border'
									: ''}"
							>
								<div
									class="flex cursor-pointer items-center gap-3 px-4 py-2.5 text-sm"
									role="button"
									tabindex="0"
									onclick={() => (expandedIndex = expanded ? null : i)}
									onkeydown={(e) => e.key === 'Enter' && (expandedIndex = expanded ? null : i)}
								>
									<span class="font-medium text-muted-foreground"
										>{SECTION_LABELS[section.type]?.()}</span
									>
									<span class="text-xs text-muted-foreground/50">
										{#if section.type === 'carousel'}{section.items.length} items · breakpoint={section.breakpoint}
										{:else if section.type === 'category'}{section.value || '—'}
										{:else if section.type === 'custom'}{section.titles[0]?.text || '—'}
										{:else if section.type === 'charts'}cards={section.cards}
										{:else if section.type === 'links'}{section.titles[0]?.text || '—'} · {section.items.length} links
										{:else}–{/if}
									</span>
								</div>
								{#if expanded}
									<div class="border-t border-border/40 px-4 py-3 text-sm">
										{#if section.type === 'carousel'}
											<div class="mb-3">
												<label class="space-y-1">
													<span class="text-xs text-muted-foreground"
														>{m.frontpage_breakpoint()}</span
													>
													<Input
														type="number"
														class="h-8 w-24 text-sm"
														bind:value={section.breakpoint}
														oninput={mark}
														min={1}
													/>
												</label>
											</div>
											{#each section.items as item, j (j)}
												<div class="mb-2 space-y-2 rounded border border-border p-3">
													<div class="flex items-center justify-between">
														<span class="text-xs tracking-wide text-muted-foreground uppercase"
															>{item.type}</span
														>
														<button
															type="button"
															onclick={() => removeCarouselItem(section, j)}
															class="text-muted-foreground hover:text-destructive"
															><Trash2 class="size-3.5" /></button
														>
													</div>
													{#if item.type === 'app'}
														<Input
															placeholder="com.example.App"
															bind:value={item.id}
															oninput={mark}
															class="h-8 font-mono text-sm"
														/>
													{:else}
														<div class="flex gap-2">
															<Input
																placeholder="banner.jpg"
																bind:value={item.banner}
																oninput={mark}
																class="h-8 flex-1 text-sm"
															/>
															<UploadButton onurl={(url) => { item.banner = url; mark(); }} />
														</div>
														{#each item.titles as t, k (k)}
															<div class="flex gap-2">
																<Input
																	class="h-8 w-14 text-sm"
																	placeholder="en"
																	bind:value={t.lang}
																	oninput={mark}
																/>
																<Input
																	class="h-8 flex-1 text-sm"
																	placeholder="Title"
																	bind:value={t.text}
																	oninput={mark}
																/>
																<button
																	type="button"
																	onclick={() => removeTitle(item.titles, k)}
																	class="text-muted-foreground hover:text-destructive"
																	><Trash2 class="size-3.5" /></button
																>
															</div>
														{/each}
														<button
															type="button"
															class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
															onclick={() => addTitle(item.titles)}
															><Plus class="size-3" /> title</button
														>
														<textarea
															class="mt-1 w-full rounded border border-input bg-muted/30 px-3 py-2 font-mono text-xs outline-none"
															rows={3}
															placeholder="Body XML…"
															bind:value={item.body}
															oninput={mark}
														></textarea>
													{/if}
												</div>
											{/each}
											<div class="flex gap-3">
												<button
													type="button"
													class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
													onclick={() => addCarouselApp(section)}
													><Plus class="size-3" /> app</button
												>
												<button
													type="button"
													class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
													onclick={() => addCarouselStory(section)}
													><Plus class="size-3" /> story</button
												>
											</div>
										{:else if section.type === 'category'}
											<Input
												placeholder="games"
												bind:value={section.value}
												oninput={mark}
												class="h-8 max-w-xs text-sm"
											/>
										{:else if section.type === 'charts'}
											<label class="flex items-center gap-2 text-sm">
												<input type="checkbox" bind:checked={section.cards} onchange={mark} />
												{m.frontpage_cards_view()}
											</label>
										{:else if section.type === 'custom'}
											<div class="space-y-3">
												<div class="space-y-2">
													<p class="text-xs text-muted-foreground">{m.frontpage_titles()}</p>
													{#each section.titles as t, k (k)}
														<div class="flex gap-2">
															<Input
																class="h-8 w-14 text-sm"
																placeholder="en"
																bind:value={t.lang}
																oninput={mark}
															/>
															<Input
																class="h-8 flex-1 text-sm"
																placeholder="Title"
																bind:value={t.text}
																oninput={mark}
															/>
															<button
																type="button"
																onclick={() => removeTitle(section.titles, k)}
																class="text-muted-foreground hover:text-destructive"
																><Trash2 class="size-3.5" /></button
															>
														</div>
													{/each}
													<button
														type="button"
														class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
														onclick={() => addTitle(section.titles)}
														><Plus class="size-3" /> title</button
													>
												</div>
												<div class="space-y-2">
													<p class="text-xs text-muted-foreground">{m.frontpage_apps_label()}</p>
													{#each section.apps, k (k)}
														<div class="flex gap-2">
															<Input
																class="h-8 flex-1 font-mono text-sm"
																placeholder="io.github.example.App"
																bind:value={section.apps[k]}
																oninput={mark}
															/>
															<button
																type="button"
																onclick={() => removeCustomApp(section, k)}
																class="text-muted-foreground hover:text-destructive"
																><Trash2 class="size-3.5" /></button
															>
														</div>
													{/each}
													<button
														type="button"
														class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
														onclick={() => addCustomApp(section)}
														><Plus class="size-3" /> app</button
													>
												</div>
											</div>
										{:else if section.type === 'links'}
											<div class="space-y-3">
												<div class="space-y-2">
													<p class="text-xs text-muted-foreground">{m.frontpage_titles()}</p>
													{#each section.titles as t, k (k)}
														<div class="flex gap-2">
															<Input
																class="h-8 w-14 text-sm"
																placeholder="en"
																bind:value={t.lang}
																oninput={mark}
															/>
															<Input
																class="h-8 flex-1 text-sm"
																placeholder="More Information"
																bind:value={t.text}
																oninput={mark}
															/>
															<button
																type="button"
																onclick={() => removeTitle(section.titles, k)}
																class="text-muted-foreground hover:text-destructive"
																><Trash2 class="size-3.5" /></button
															>
														</div>
													{/each}
													<button
														type="button"
														class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
														onclick={() => addTitle(section.titles)}
														><Plus class="size-3" /> title</button
													>
												</div>
												<div class="space-y-2">
													<p class="text-xs text-muted-foreground">{m.frontpage_links_items()}</p>
													{#each section.items as item, k (k)}
														<div class="space-y-1.5 rounded border border-border p-2.5">
															<div class="flex items-center justify-between">
																<span class="text-xs tracking-wide text-muted-foreground uppercase">{item.kind}</span>
																<button
																	type="button"
																	onclick={() => removeLinksItem(section, k)}
																	class="text-muted-foreground hover:text-destructive"
																	><Trash2 class="size-3.5" /></button
																>
															</div>
															{#if item.kind === 'url'}
																<Input
																	class="h-8 text-sm"
																	placeholder={m.frontpage_links_text()}
																	bind:value={item.text}
																	oninput={mark}
																/>
																<Input
																	class="h-8 font-mono text-sm"
																	placeholder="https://example.com"
																	bind:value={item.href}
																	oninput={mark}
																/>
															{:else if item.kind === 'app'}
																<Input
																	class="h-8 font-mono text-sm"
																	placeholder="com.example.App"
																	bind:value={item.id}
																	oninput={mark}
																/>
															{:else}
																<div class="flex gap-2">
																	<Input
																		placeholder="banner.jpg"
																		bind:value={item.banner}
																		oninput={mark}
																		class="h-8 flex-1 text-sm"
																	/>
																	<UploadButton onurl={(url) => { item.banner = url; mark(); }} />
																</div>
																{#each item.titles as t, j (j)}
																	<div class="flex gap-2">
																		<Input
																			class="h-8 w-14 text-sm"
																			placeholder="en"
																			bind:value={t.lang}
																			oninput={mark}
																		/>
																		<Input
																			class="h-8 flex-1 text-sm"
																			placeholder="Title"
																			bind:value={t.text}
																			oninput={mark}
																		/>
																		<button
																			type="button"
																			onclick={() => removeTitle(item.titles, j)}
																			class="text-muted-foreground hover:text-destructive"
																			><Trash2 class="size-3.5" /></button
																		>
																	</div>
																{/each}
																<button
																	type="button"
																	class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
																	onclick={() => addTitle(item.titles)}
																	><Plus class="size-3" /> title</button
																>
																<textarea
																	class="mt-1 w-full rounded border border-input bg-muted/30 px-3 py-2 font-mono text-xs outline-none"
																	rows={3}
																	placeholder="Body XML…"
																	bind:value={item.body}
																	oninput={mark}
																></textarea>
															{/if}
														</div>
													{/each}
													<div class="flex gap-3">
														<button
															type="button"
															class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
															onclick={() => addLinksUrl(section)}
															><Plus class="size-3" /> URL</button
														>
														<button
															type="button"
															class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
															onclick={() => addLinksApp(section)}
															><Plus class="size-3" /> app</button
														>
														<button
															type="button"
															class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
															onclick={() => addLinksStory(section)}
															><Plus class="size-3" /> story</button
														>
													</div>
												</div>
											</div>
										{:else}
											<p class="text-xs text-muted-foreground italic">{m.frontpage_no_config()}</p>
										{/if}
									</div>
								{/if}
							</div>
						{/if}
					</div>
				</div>
			{/each}

			<div
				class="mt-1 cursor-text py-2 pl-14 text-sm text-muted-foreground/20 transition-colors hover:text-muted-foreground/40"
				role="button"
				tabindex="0"
				onclick={() => insertParagraph(sections.length - 1)}
				onkeydown={(e) => {
					if (e.key === 'Enter') {
						e.preventDefault();
						insertParagraph(sections.length - 1);
					}
				}}
			>
				{m.frontpage_press()} <span class="font-mono">Enter</span>
				{m.frontpage_to_continue()}
				<span class="font-mono">/</span>
				{m.frontpage_for_a_block()}
			</div>
		{/if}
	</div>
</form>

{#if paletteOpen}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-40"
		onclick={closePalette}
		onkeydown={(e) => e.key === 'Escape' && closePalette()}
	></div>
	<div
		class="fixed z-50 w-64 overflow-hidden rounded-xl border border-border bg-background shadow-2xl"
		style="top:{paletteY}px;left:{paletteX}px"
	>
		<!-- Header -->
		<div class="px-3 pt-2.5 pb-1">
			<p class="text-[10px] font-medium tracking-wider text-muted-foreground/50 uppercase">
				{paletteFilter ? m.frontpage_filtered() : m.frontpage_blocks()}
			</p>
		</div>

		<!-- Items -->
		<ul class="max-h-72 overflow-y-auto pb-1">
			{#each filtered as cmd, i (cmd.type)}
				<li>
					<button
						type="button"
						class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm {selectedCmd === i
							? 'bg-muted'
							: 'hover:bg-muted'}"
						onclick={() => pick(cmd.type)}
						onmouseenter={() => (selectedCmd = i)}
					>
						<div
							class="flex size-6 shrink-0 items-center justify-center rounded border border-border bg-muted/60"
						>
							{#if cmd.badge}
								<span class="text-[10px] leading-none font-bold text-foreground/70"
									>{cmd.badge}</span
								>
							{:else}
								<cmd.icon class="size-3 text-muted-foreground" />
							{/if}
						</div>
						<span class="flex-1 font-medium">{cmd.label}</span>
						{#if cmd.shorthand}
							<span class="font-mono text-[11px] text-muted-foreground/40">{cmd.shorthand}</span>
						{/if}
					</button>
				</li>
			{/each}
			{#if filtered.length === 0}
				<li class="px-3 py-4 text-center text-xs text-muted-foreground">
					{m.frontpage_no_results()}
				</li>
			{/if}
		</ul>

		<!-- Footer -->
		<div class="flex items-center justify-between border-t border-border/60 px-3 py-2">
			<button
				type="button"
				onclick={closePalette}
				class="text-xs text-muted-foreground/60 hover:text-foreground"
			>
				{m.frontpage_close_menu()}
			</button>
			<kbd
				class="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground/60"
				>esc</kbd
			>
		</div>
	</div>
{/if}
