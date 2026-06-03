import { Client } from 'basic-ftp';
import { Readable } from 'stream';
import { env } from '$env/dynamic/private';

const HOST = 'storage.bunnycdn.com';
const USER = 'blossomos';
const REMOTE_DIR = '/forgeassets/';
const CDN_BASE = 'https://cdn.blossomos.org/forgeassets/';

export async function uploadFile(data: ArrayBuffer, filename: string): Promise<string> {
	const client = new Client();
	client.ftp.timeout = 30_000;
	try {
		await client.access({
			host: HOST,
			port: 21,
			user: USER,
			password: env.BUNNYCDN_PASSWORD,
			secure: false
		});
		await client.uploadFrom(Readable.from(Buffer.from(data)), `${REMOTE_DIR}${filename}`);
		return `${CDN_BASE}${filename}`;
	} finally {
		client.close();
	}
}
