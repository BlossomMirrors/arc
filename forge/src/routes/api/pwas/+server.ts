import { db } from '$lib/server/db';
import { applyLang, parseLang } from '$lib/server/pwa';
import type { RequestHandler } from './$types';

function toPublic(app: ReturnType<typeof applyLang>) {
	return {
		id: app.appid,
		appid: app.appid,
		name: app.name,
		summary: app.summary,
		description: app.description,
		icon_url: app.iconUrl,
		screenshots: app.screenshots,
		homepage_url: app.homepageUrl,
		content_rating: app.contentRating,
		developer_name: app.developerName,
		verified: true,
		url: app.url,
		color: app.color,
		css: app.css,
		js: app.js,
		useragent: app.useragent,
		widevine: app.widevine,
		tray: app.tray
	};
}

export const GET: RequestHandler = async ({ url }) => {
	const lang = parseLang(url);
	const apps = await db.pwaApp.findMany({
		orderBy: { createdAt: 'asc' },
		include: { translations: true }
	});
	return Response.json(apps.map((a) => toPublic(applyLang(a, lang))));
};
