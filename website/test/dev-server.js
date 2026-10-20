// The site and its API on http://localhost:3333 with an in-memory database
// and a few sample words, for working on admin.html:
//   node test/dev-server.js            password "dev"
//   node test/dev-server.js --signed-in  every request as the admin

import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { extname, join, normalize } from 'node:path';

import { PGlite } from '@electric-sql/pglite';

import { hashPassword, useDatabase } from '../api/_lib.js';

const ROOT = new URL('..', import.meta.url).pathname;
const PORT = 3333;
const signedIn = process.argv.includes('--signed-in');

const db = new PGlite();
useDatabase(async (text, params) => (await db.query(text, params)).rows);
process.env.ADMIN_PASSWORD_HASH = hashPassword('dev', 'dev0');
process.env.SESSION_SECRET = 'dev-secret';

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.svg': 'image/svg+xml', '.css': 'text/css' };

async function api(req, res, path) {
  const mod = await import(join(ROOT, path) + '.js');
  let raw = '';
  for await (const chunk of req) raw += chunk;
  const url = new URL(req.url, `http://localhost:${PORT}`);
  req.body = raw ? JSON.parse(raw) : undefined;
  req.query = Object.fromEntries(url.searchParams);
  res.status = (code) => { res.statusCode = code; return res; };
  res.json = (data) => { res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify(data)); };
  await mod.default(req, res);
}

const server = createServer(async (req, res) => {
  const path = new URL(req.url, `http://localhost:${PORT}`).pathname;
  if (signedIn) {
    const { startSession } = await import('../api/_lib.js');
    const fake = { setHeader: (_, v) => { req.headers.cookie = v.split(';')[0]; } };
    startSession(fake);
  }
  if (path.startsWith('/api/')) return api(req, res, normalize(path).replace(/^\//, ''));
  const file = join(ROOT, normalize(path === '/' ? '/index.html' : path));
  try {
    const body = await readFile(file);
    res.setHeader('Content-Type', TYPES[extname(file)] || 'application/octet-stream');
    res.end(body);
  } catch {
    res.statusCode = 404;
    res.end('not found');
  }
});

// Sample words from three devices.
const { default: upload } = await import('../api/words.js');
const send = (device, words) => new Promise((resolve) => upload(
  { method: 'POST', body: { device, app: '3.0', words }, headers: {} },
  { setHeader() {}, status() { return this; }, end: resolve, json: resolve },
));
await send('00000000-0000-4000-8000-000000000001', [
  { text: '貢丸湯', typed: 'ㄍㄨㄥㄨㄢㄊ', count: 4 }, { text: '我想吃飯', typed: 'ㄨㄒㄔㄈ', count: 2 },
]);
await send('00000000-0000-4000-8000-000000000002', [
  { text: '貢丸湯', typed: 'ㄍㄨㄊ', count: 1 }, { text: '陳小美', typed: 'ㄔㄒㄇ', count: 9 },
]);
await send('00000000-0000-4000-8000-000000000003', [{ text: '貢丸湯', typed: 'ㄍㄨㄊ', count: 2 }]);

server.listen(PORT, () => console.log(`http://localhost:${PORT}/admin.html${signedIn ? ' (signed in)' : ''}`));
