import { fail } from '@sveltejs/kit';
import { PrismaClient } from '$lib/generated/prisma/client';
import { PrismaPg } from '@prisma/adapter-pg';
import { DATABASE_URL } from '$env/static/private';
import type { Actions, PageServerLoad } from './$types';

const prisma = new PrismaClient({ adapter: new PrismaPg({ connectionString: DATABASE_URL }) });

export const load: PageServerLoad = async () => {
	const entries = await prisma.whitelistEntry.findMany({ orderBy: { createdAt: 'asc' } });
	return { entries };
};

export const actions: Actions = {
	add: async ({ request }) => {
		const data = await request.formData();
		const value = (data.get('value') as string)?.trim();
		if (!value) return fail(400, { error: 'Value is required' });

		await prisma.whitelistEntry.upsert({
			where: { value },
			update: {},
			create: { value }
		});
	},

	remove: async ({ request }) => {
		const data = await request.formData();
		const id = data.get('id') as string;
		if (!id) return fail(400, { error: 'ID is required' });
		await prisma.whitelistEntry.delete({ where: { id } });
	}
};
