import { PrismaClient } from '$lib/generated/prisma/client';
import { PrismaPg } from '@prisma/adapter-pg';
import { env } from '$env/dynamic/private';

export const db = new PrismaClient({ adapter: new PrismaPg({ connectionString: env.DATABASE_URL }) });
