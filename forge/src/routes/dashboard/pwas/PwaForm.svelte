<script lang="ts">
	import { untrack } from 'svelte';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Button, buttonVariants } from '$lib/components/ui/button/index.js';
	import * as m from '$lib/paraglide/messages';
	import { CONTENT_LANGS } from '$lib/content-langs';
	import UploadButton from '$lib/components/upload-button.svelte';

	type PwaFormData = {
		appid?: string;
		name?: string;
		summary?: string;
		description?: string;
		iconUrl?: string;
		screenshots?: string[];
		homepageUrl?: string;
		contentRating?: string;
		developerName?: string;
		url?: string;
		color?: string;
		css?: string;
		js?: string;
		useragent?: string;
		widevine?: boolean;
		tray?: boolean;
	};

	type Translation = { name?: string | null; summary?: string | null; description?: string | null };

	let {
		values = {},
		translations = {},
		submitLabel = 'Save'
	}: {
		values?: PwaFormData;
		translations?: Record<string, Translation>;
		submitLabel?: string;
	} = $props();

	let screenshots = $state(untrack(() => (values.screenshots ?? []).join('\n')));
	let widevine    = $state(untrack(() => values.widevine ?? false));
	let tray        = $state(untrack(() => values.tray ?? false));
	let iconUrl     = $state(untrack(() => values.iconUrl ?? ''));
</script>

<div class="space-y-4">
	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_app_id()}</span>
			<Input name="appid" value={values.appid ?? ''} placeholder="com.example.App" required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_name()}</span>
			<Input name="name" value={values.name ?? ''} placeholder="My App" required />
		</label>
	</div>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">{m.form_summary()}</span>
		<Input name="summary" value={values.summary ?? ''} placeholder="Short description" required />
	</label>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">{m.form_description()}</span>
		<textarea
			name="description"
			rows={4}
			class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder="<p>Full description...</p>"
			>{values.description ?? ''}</textarea
		>
	</label>

	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_icon_url()}</span>
			<div class="flex gap-2">
				<Input name="iconUrl" bind:value={iconUrl} placeholder="https://..." required class="flex-1" />
				<UploadButton onurl={(url) => (iconUrl = url)} />
			</div>
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_homepage_url()}</span>
			<Input name="homepageUrl" value={values.homepageUrl ?? ''} placeholder="https://..." />
		</label>
	</div>

	<div class="space-y-1.5">
		<div class="flex items-center justify-between">
			<span class="text-sm font-medium">{m.form_screenshots()}</span>
			<UploadButton onurl={(url) => (screenshots = screenshots ? screenshots + '\n' + url : url)} />
		</div>
		<textarea
			name="screenshots"
			rows={3}
			class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder="https://..."
			bind:value={screenshots}
		></textarea>
	</div>

	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_developer_name()}</span>
			<Input name="developerName" value={values.developerName ?? ''} placeholder="ACME Corp" required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_content_rating()}</span>
			<Input name="contentRating" value={values.contentRating ?? 'All ages'} placeholder="All ages" />
		</label>
	</div>

	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_url()}</span>
			<Input name="url" value={values.url ?? ''} placeholder="https://..." required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">{m.form_theme_color()}</span>
			<Input name="color" type="color" value={values.color ?? '#000000'} />
		</label>
	</div>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">{m.form_user_agent()}</span>
		<Input name="useragent" value={values.useragent ?? ''} placeholder="Mozilla/5.0..." />
	</label>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">{m.form_custom_css()}</span>
		<textarea
			name="css"
			rows={4}
			class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder="body &#123; color: red; &#125;"
			>{values.css ?? ''}</textarea
		>
	</label>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">{m.form_custom_js()}</span>
		<textarea
			name="js"
			rows={4}
			class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder="console.log('hello')"
			>{values.js ?? ''}</textarea
		>
	</label>

	<div class="flex gap-6">
		<label class="flex items-center gap-2 text-sm">
			<input type="checkbox" name="widevine" value="true" bind:checked={widevine} class="rounded border-input" />
			{m.form_widevine()}
		</label>
		<label class="flex items-center gap-2 text-sm">
			<input type="checkbox" name="tray" value="true" bind:checked={tray} class="rounded border-input" />
			{m.form_tray()}
		</label>
	</div>

	<div class="space-y-4 border-t border-border pt-4">
		<div>
			<p class="text-sm font-medium">{m.form_translations()}</p>
			<p class="text-xs text-muted-foreground">{m.form_translation_fallback_note()}</p>
		</div>
		{#each CONTENT_LANGS as lang (lang.code)}
			<details class="rounded-lg border border-border">
				<summary class="cursor-pointer px-4 py-2.5 text-sm font-medium select-none">
					{lang.label}
				</summary>
				<div class="space-y-3 px-4 pb-4 pt-3">
					<label class="space-y-1.5">
						<span class="text-sm font-medium">{m.form_name()}</span>
						<Input
							name="trans_{lang.code}_name"
							value={translations[lang.code]?.name ?? ''}
							placeholder={values.name ?? ''}
						/>
					</label>
					<label class="space-y-1.5">
						<span class="text-sm font-medium">{m.form_summary()}</span>
						<Input
							name="trans_{lang.code}_summary"
							value={translations[lang.code]?.summary ?? ''}
							placeholder={values.summary ?? ''}
						/>
					</label>
					<label class="space-y-1.5">
						<span class="text-sm font-medium">{m.form_description()}</span>
						<textarea
							name="trans_{lang.code}_description"
							rows={3}
							class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
							placeholder={values.description ?? ''}
						>{translations[lang.code]?.description ?? ''}</textarea>
					</label>
				</div>
			</details>
		{/each}
	</div>

	<div class="flex gap-2 pt-2">
		<Button type="submit">{submitLabel}</Button>
		<a href="/dashboard/pwas" class={buttonVariants({ variant: 'ghost' })}>{m.form_cancel()}</a>
	</div>
</div>
