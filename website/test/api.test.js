// The API against an in-memory Postgres (PGlite): npm test

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { PGlite } from '@electric-sql/pglite';

import { hashPassword, useDatabase } from '../api/_lib.js';
import exportWords from '../api/admin/export.js';
import login from '../api/admin/login.js';
import adminWords from '../api/admin/words.js';
import upload from '../api/words.js';

const db = new PGlite();
useDatabase(async (text, params) => (await db.query(text, params)).rows);
process.env.ADMIN_PASSWORD_HASH = hashPassword('right horse', 'aa11');
process.env.SESSION_SECRET = 'test-secret';

/** Call a handler like Vercel would; resolves to {status, body, headers}. */
function call(fn, { method = 'GET', body, query = {}, cookie } = {}) {
  return new Promise((resolve) => {
    const headers = {};
    const res = {
      statusCode: 200,
      setHeader: (k, v) => { headers[k.toLowerCase()] = v; },
      status(code) { this.statusCode = code; return this; },
      json(data) { resolve({ status: this.statusCode, body: data, headers }); },
      end() { resolve({ status: this.statusCode, body: undefined, headers }); },
    };
    fn({ method, body, query, headers: cookie ? { cookie } : {} }, res);
  });
}

const device = (n) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;

test('uploads keep each device\'s highest count', async () => {
  const words = [{ text: '貢丸湯', typed: 'ㄍㄨㄥㄨㄢㄊ', count: 2 }];
  assert.equal((await call(upload, { method: 'POST', body: { device: device(1), app: '3.0', words } })).status, 204);
  words[0].count = 5;
  await call(upload, { method: 'POST', body: { device: device(1), app: '3.0', words } });
  words[0].count = 1;
  await call(upload, { method: 'POST', body: { device: device(1), app: '3.0', words } });
  await call(upload, { method: 'POST', body: { device: device(2), app: '3.0', words: [{ text: '貢丸湯', typed: 'ㄍㄨㄊ', count: 3 }] } });
  const rows = await db.query("SELECT device, typed, count FROM submissions WHERE text = '貢丸湯' ORDER BY device");
  assert.deepEqual(rows.rows.map((r) => [r.typed, r.count]), [['ㄍㄨㄥㄨㄢㄊ', 5], ['ㄍㄨㄊ', 3]]);
});

test('uploads are checked', async () => {
  const bad = [
    { device: 'not-a-uuid', words: [] },
    { device: device(3), words: [{ text: 'abc', typed: 'ㄅ', count: 1 }] },
    { device: device(3), words: [{ text: '你好', typed: 'nh', count: 1 }] },
    { device: device(3), words: [{ text: '你好', typed: 'ㄋㄏ', count: 0 }] },
    { device: device(3), words: Array(501).fill({ text: '你好', typed: 'ㄋㄏ', count: 1 }) },
  ];
  for (const body of bad) {
    assert.equal((await call(upload, { method: 'POST', body })).status, 400, JSON.stringify(body).slice(0, 80));
  }
  assert.equal((await call(upload, { method: 'GET' })).status, 405);
});

test('only the admin signs in and reviews', async () => {
  assert.equal((await call(adminWords)).status, 401, 'no session');
  assert.equal((await call(adminWords, { cookie: 'lb_admin=99999999999999.forged' })).status, 401, 'forged');
  const wrong = await call(login, { method: 'POST', body: { email: 'ino@noky.dev', password: 'wrong' } });
  assert.equal(wrong.status, 401);
  const other = await call(login, { method: 'POST', body: { email: 'x@noky.dev', password: 'right horse' } });
  assert.equal(other.status, 401);
  const ok = await call(login, { method: 'POST', body: { email: ' Ino@Noky.dev ', password: 'right horse' } });
  assert.equal(ok.status, 204);
  const cookie = ok.headers['set-cookie'].split(';')[0];
  assert.match(ok.headers['set-cookie'], /HttpOnly; Secure; SameSite=Strict/);

  const pending = await call(adminWords, { cookie });
  assert.equal(pending.status, 200);
  const soup = pending.body.words.find((w) => w.text === '貢丸湯');
  assert.equal(soup.devices, 2);
  assert.equal(soup.uses, 8);
  assert.deepEqual([...soup.typed].sort(), ['ㄍㄨㄊ', 'ㄍㄨㄥㄨㄢㄊ']);

  assert.equal((await call(adminWords, { method: 'POST', cookie, body: { text: '貢丸湯', status: 'approved' } })).status, 204);
  assert.equal((await call(adminWords, { cookie, query: { status: 'pending' } })).body.words.length, 0);
  const approved = await call(adminWords, { cookie, query: { status: 'approved' } });
  assert.deepEqual(approved.body.words.map((w) => w.text), ['貢丸湯']);
  assert.equal(approved.body.counts.approved, 1);

  const exported = await call(exportWords, { cookie });
  assert.deepEqual(exported.body.words.map((w) => [w.text, w.typed, w.uses]), [['貢丸湯', 'ㄍㄨㄥㄨㄢㄊ', 5], ['貢丸湯', 'ㄍㄨㄊ', 3]]);

  assert.equal((await call(adminWords, { method: 'POST', cookie, body: { text: '貢丸湯', status: 'pending' } })).status, 204);
  assert.equal((await call(adminWords, { cookie })).body.words.length, 1, 'undone');
  assert.equal((await call(exportWords)).status, 401);
});
