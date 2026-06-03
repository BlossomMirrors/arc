import { fail, redirect } from '@sveltejs/kit';
import { db } from '$lib/server/db';
import { saveTranslations } from '$lib/server/pwa-form';
import type { Actions } from './$types';

function parseForm(data: FormData) {
	return {
		appid: (data.get('appid') as string).trim(),
		name: (data.get('name') as string).trim(),
		summary: (data.get('summary') as string).trim(),
		description: (data.get('description') as string).trim(),
		iconUrl: (data.get('iconUrl') as string).trim(),
		screenshots: (data.get('screenshots') as string)
			.split('\n')
			.map((s) => s.trim())
			.filter(Boolean),
		homepageUrl: (data.get('homepageUrl') as string).trim(),
		contentRating: (data.get('contentRating') as string).trim() || 'All ages',
		developerName: (data.get('developerName') as string).trim(),
		url: (data.get('url') as string).trim(),
		color: (data.get('color') as string).trim() || '#000000',
		css: (data.get('css') as string).trim(),
		js: (data.get('js') as string).trim(),
		useragent: (data.get('useragent') as string).trim(),
		widevine: data.get('widevine') === 'true',
		tray: data.get('tray') === 'true'
	};
}

export const actions: Actions = {
	default: async ({ request }) => {
		const data = await request.formData();
		const fields = parseForm(data);
		if (!fields.appid || !fields.name) return fail(400, { error: 'Missing required fields' });

		const app = await db.pwaApp.create({ data: fields });
		await saveTranslations(app.id, data);
		throw redirect(303, '/dashboard/pwas');
	}
};
