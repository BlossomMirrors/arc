import { db } from '$lib/server/db';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async () => {
	const entries = await db.whitelistEntry.findMany({ orderBy: { createdAt: 'asc' } });
	return new Response(entries.map((e) => e.value).join('\n'), {
		headers: { 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'no-store' }
	});
};
