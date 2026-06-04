import { fail, redirect, error } from '@sveltejs/kit';
import { db } from '$lib/server/db';
import { saveTranslations, uploadCodeField } from '$lib/server/pwa-form';
import type { Actions, PageServerLoad } from './$types';

async function parseForm(data: FormData) {
	const css = (data.get('css') as string).trim();
	const js = (data.get('js') as string).trim();
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
		css: await uploadCodeField(css, 'css'),
		js: await uploadCodeField(js, 'js'),
		useragent: (data.get('useragent') as string).trim(),
		widevine: data.get('widevine') === 'true',
		tray: data.get('tray') === 'true'
	};
}

export const load: PageServerLoad = async ({ params }) => {
	const app = await db.pwaApp.findUnique({
		where: { id: params.id },
		include: { translations: true }
	});
	if (!app) throw error(404, 'PWA not found');

	const translations = Object.fromEntries(
		app.translations.map((t) => [t.lang, { name: t.name, summary: t.summary, description: t.description }])
	);
	return { app, translations };
};

export const actions: Actions = {
	default: async ({ request, params }) => {
		const data = await request.formData();
		const fields = await parseForm(data);
		if (!fields.appid || !fields.name) return fail(400, { error: 'Missing required fields' });

		await db.pwaApp.update({ where: { id: params.id }, data: fields });
		await saveTranslations(params.id, data);
		throw redirect(303, '/dashboard/pwas');
	}
};
