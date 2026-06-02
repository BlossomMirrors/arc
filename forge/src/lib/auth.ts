import { betterAuth } from 'better-auth';
import { prismaAdapter } from 'better-auth/adapters/prisma';
import { genericOAuth } from 'better-auth/plugins';
import { sveltekitCookies } from 'better-auth/svelte-kit';
import { getRequestEvent } from '$app/server';
import { PrismaClient } from '$lib/generated/prisma/client';
import { PrismaPg } from '@prisma/adapter-pg';
import {
	DATABASE_URL,
	BETTER_AUTH_URL,
	AUTHENTIK_CLIENT_ID,
	AUTHENTIK_CLIENT_SECRET,
	AUTHENTIK_URL
} from '$env/static/private';

const pgAdapter = new PrismaPg({ connectionString: DATABASE_URL });
const prisma = new PrismaClient({ adapter: pgAdapter });

export const auth = betterAuth({
	database: prismaAdapter(prisma, {
		provider: 'postgresql'
	}),
	baseURL: BETTER_AUTH_URL,
	plugins: [
		genericOAuth({
			config: [
				{
					providerId: 'blossom-accounts',
					clientId: AUTHENTIK_CLIENT_ID,
					clientSecret: AUTHENTIK_CLIENT_SECRET,
					discoveryUrl: `${AUTHENTIK_URL}/application/o/arc-forge/.well-known/openid-configuration`,
					scopes: ['openid', 'email', 'profile']
				}
			]
		}),
		sveltekitCookies(getRequestEvent)
	]
});
