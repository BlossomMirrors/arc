import { db } from '$lib/server/db';
import { sectionsToXml } from '$lib/frontpage';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async () => {
	const row = await db.frontPage.findUnique({ where: { id: 'singleton' } });
	const sections = (row?.sections as unknown[]) ?? [];
	return new Response(sectionsToXml(sections as Parameters<typeof sectionsToXml>[0]), {
		headers: { 'Content-Type': 'application/xml; charset=utf-8', 'Cache-Control': 'no-store' }
	});
};
