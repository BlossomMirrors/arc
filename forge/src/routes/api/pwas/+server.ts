import { db } from '$lib/server/db';
import type { RequestHandler } from './$types';

function toPublic(app: Awaited<ReturnType<typeof db.pwaApp.findMany>>[number]) {
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

export const GET: RequestHandler = async () => {
	const apps = await db.pwaApp.findMany({ orderBy: { createdAt: 'asc' } });
	return Response.json(apps.map(toPublic));
};
