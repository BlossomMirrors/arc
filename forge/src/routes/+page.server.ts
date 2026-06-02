import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ locals }) => {
	throw redirect(302, locals.user ? '/dashboard' : '/auth/login');
};
