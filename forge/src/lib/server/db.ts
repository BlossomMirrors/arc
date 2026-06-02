import { PrismaClient } from '$lib/generated/prisma/client';
import { PrismaPg } from '@prisma/adapter-pg';
import { DATABASE_URL } from '$env/static/private';

export const db = new PrismaClient({ adapter: new PrismaPg({ connectionString: DATABASE_URL }) });
