<script lang="ts">
	import { Input } from '$lib/components/ui/input/index.js';
	import { Button, buttonVariants } from '$lib/components/ui/button/index.js';

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

	let { values = {}, submitLabel = 'Save' }: { values?: PwaFormData; submitLabel?: string } =
		$props();

	let screenshots = $state((values.screenshots ?? []).join('\n'));
	let widevine = $state(values.widevine ?? false);
	let tray = $state(values.tray ?? false);
</script>

<div class="space-y-4">
	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">App ID</span>
			<Input name="appid" value={values.appid ?? ''} placeholder="com.example.App" required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">Name</span>
			<Input name="name" value={values.name ?? ''} placeholder="My App" required />
		</label>
	</div>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">Summary</span>
		<Input name="summary" value={values.summary ?? ''} placeholder="Short description" required />
	</label>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">Description (HTML)</span>
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
			<span class="text-sm font-medium">Icon URL</span>
			<Input name="iconUrl" value={values.iconUrl ?? ''} placeholder="https://..." required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">Homepage URL</span>
			<Input name="homepageUrl" value={values.homepageUrl ?? ''} placeholder="https://..." />
		</label>
	</div>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">Screenshots (one URL per line)</span>
		<textarea
			name="screenshots"
			rows={3}
			class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder="https://..."
			bind:value={screenshots}
		></textarea>
	</label>

	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">Developer Name</span>
			<Input name="developerName" value={values.developerName ?? ''} placeholder="ACME Corp" required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">Content Rating</span>
			<Input name="contentRating" value={values.contentRating ?? 'All ages'} placeholder="All ages" />
		</label>
	</div>

	<div class="grid grid-cols-2 gap-4">
		<label class="space-y-1.5">
			<span class="text-sm font-medium">URL</span>
			<Input name="url" value={values.url ?? ''} placeholder="https://..." required />
		</label>
		<label class="space-y-1.5">
			<span class="text-sm font-medium">Theme Color</span>
			<Input name="color" type="color" value={values.color ?? '#000000'} />
		</label>
	</div>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">User Agent</span>
		<Input name="useragent" value={values.useragent ?? ''} placeholder="Mozilla/5.0..." />
	</label>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">Custom CSS</span>
		<textarea
			name="css"
			rows={4}
			class="w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
			placeholder="body &#123; color: red; &#125;"
			>{values.css ?? ''}</textarea
		>
	</label>

	<label class="space-y-1.5">
		<span class="text-sm font-medium">Custom JS</span>
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
			Widevine DRM
		</label>
		<label class="flex items-center gap-2 text-sm">
			<input type="checkbox" name="tray" value="true" bind:checked={tray} class="rounded border-input" />
			System Tray
		</label>
	</div>

	<div class="flex gap-2 pt-2">
		<Button type="submit">{submitLabel}</Button>
		<a href="/dashboard/pwas" class={buttonVariants({ variant: 'ghost' })}>Cancel</a>
	</div>
</div>
