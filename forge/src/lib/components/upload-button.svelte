<script lang="ts">
	import { Upload } from '@lucide/svelte';

	let { onurl }: { onurl: (url: string) => void } = $props();

	let input: HTMLInputElement;
	let uploading = $state(false);
	let error = $state('');

	async function handleChange(e: Event) {
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		uploading = true;
		error = '';
		try {
			const body = new FormData();
			body.append('file', file);
			const res = await fetch('/api/upload', { method: 'POST', body });
			if (!res.ok) { error = await res.text(); return; }
			const { url } = await res.json();
			onurl(url);
		} catch {
			error = 'Upload failed';
		} finally {
			uploading = false;
			input.value = '';
		}
	}
</script>

<input bind:this={input} type="file" accept="image/*" class="hidden" onchange={handleChange} />

<button
	type="button"
	onclick={() => input.click()}
	disabled={uploading}
	class="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-input bg-background px-3 py-2 text-sm text-muted-foreground transition hover:bg-muted disabled:opacity-50"
	title={error || undefined}
>
	<Upload class="size-3.5" />
	{uploading ? '…' : 'Upload'}
</button>
