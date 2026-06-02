import { PrismaClient } from '$lib/generated/prisma/client';
import { PrismaPg } from '@prisma/adapter-pg';
import { DATABASE_URL } from '$env/static/private';
import type { RequestHandler } from './$types';

const prisma = new PrismaClient({ adapter: new PrismaPg({ connectionString: DATABASE_URL }) });

export const GET: RequestHandler = async () => {
	const entries = await prisma.whitelistEntry.findMany({
		orderBy: { createdAt: 'asc' }
	});

	const body = entries.map((e) => e.value).join('\n');

	return new Response(body, {
		headers: {
			'Content-Type': 'text/plain; charset=utf-8',
			'Cache-Control': 'no-store'
		}
	});
};
