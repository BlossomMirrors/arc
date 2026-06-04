import { Client } from 'basic-ftp';
import { Readable } from 'stream';
import { env } from '$env/dynamic/private';

const HOST = 'storage.bunnycdn.com';
const USER = 'blossomos';
const REMOTE_DIR = '/forgeassets/';
export const CDN_BASE = 'https://cdn.blossomos.org/forgeassets/';

async function upload(buffer: Buffer, filename: string): Promise<string> {
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
		await client.uploadFrom(Readable.from(buffer), `${REMOTE_DIR}${filename}`);
		return `${CDN_BASE}${filename}`;
	} finally {
		client.close();
	}
}

export async function uploadFile(data: ArrayBuffer, filename: string): Promise<string> {
	return upload(Buffer.from(data), filename);
}

export async function uploadText(content: string, ext: string): Promise<string> {
	const filename = `${crypto.randomUUID()}.${ext}`;
	return upload(Buffer.from(content, 'utf-8'), filename);
}
