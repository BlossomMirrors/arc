import { db } from '$lib/server/db';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request }) => {
	let body: unknown;
	try {
		body = await request.json();
	} catch {
		return new Response('Bad Request', { status: 400 });
	}

	const appid = (body as Record<string, unknown>)?.appid;
	if (!appid || typeof appid !== 'string') {
		return new Response('Missing appid', { status: 400 });
	}

	await db.appInstall.create({ data: { appid } });
	return Response.json({ ok: true });
};
