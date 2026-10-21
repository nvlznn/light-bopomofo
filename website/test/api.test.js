// The API against an in-memory Postgres (PGlite): npm test

import assert from 'node:assert/strict';
import { after, test } from 'node:test';

import { PGlite } from '@electric-sql/pglite';

import { hashPassword, useDatabase } from '../api/_lib.js';
import exportWords from '../api/admin/export.js';
import login from '../api/admin/login.js';
import adminWords from '../api/admin/words.js';
import upload from '../api/words.js';

const db = new PGlite();
after(() => db.close());
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

test('review filter defaults to two uses and supports custom totals across devices and spellings', async () => {
  const signed = await call(login, { method: 'POST', body: { email: 'ino@noky.dev', password: 'right horse' } });
  const cookie = signed.headers['set-cookie'].split(';')[0];
  const before = await call(adminWords, { cookie });
  const send = (n, text, count, typed = 'ㄒㄘ') => call(upload, {
    method: 'POST', body: { device: device(n), app: '3.1', words: [{ text, typed, count }] },
  });
  await send(10, '一次新詞', 1);
  await send(10, '一次新詞', 1);
  await send(11, '同機兩次', 2);
  await send(12, '跨機兩次', 1);
  await send(13, '跨機兩次', 1);
  await send(14, '不同注音', 1, 'ㄅㄊ');
  await send(14, '不同注音', 1, 'ㄅㄊㄓㄧ');
  const pending = await call(adminWords, { cookie });
  assert.equal(pending.body.words.some(w => w.text === '一次新詞'), false);
  for (const text of ['同機兩次', '跨機兩次', '不同注音']) {
    assert.equal(pending.body.words.find(w => w.text === text)?.uses, 2, text);
  }
  assert.equal(pending.body.counts.pending, before.body.counts.pending + 3);
  const all = await call(adminWords, { cookie, query: { minUses: '1' } });
  assert.equal(all.body.words.some(w => w.text === '一次新詞'), true);
  assert.equal(all.body.counts.pending, before.body.counts.pending + 4);
  const high = await call(adminWords, { cookie, query: { minUses: '3' } });
  assert.equal(high.body.words.every(w => w.uses >= 3), true);
  assert.equal(high.body.counts.pending, before.body.counts.pending);
  for (const minUses of ['0', '-1', '1.5', 'abc', '2147483648', '']) {
    assert.equal((await call(adminWords, { cookie, query: { minUses } })).status, 400);
  }
  for (const status of ['approved', 'rejected']) {
    const result = await call(adminWords, { method: 'POST', cookie, body: { text: '不存在的詞', status } });
    assert.equal(result.status, 400);
  }
  assert.equal((await call(adminWords, { method: 'POST', cookie, body: { text: '一次新詞', status: 'approved' } })).status, 204);
  const filteredApproved = await call(adminWords, { cookie, query: { status: 'approved' } });
  assert.equal(filteredApproved.body.words.length, 0);
  assert.equal(filteredApproved.body.counts.approved || 0, 0);
  const allApproved = await call(adminWords, { cookie, query: { status: 'approved', minUses: '1' } });
  assert.equal(allApproved.body.words.length, 1);
  assert.equal(allApproved.body.counts.approved, 1);
  await call(adminWords, { method: 'POST', cookie, body: { text: '一次新詞', status: 'pending' } });
  assert.equal((await db.query("SELECT * FROM decisions WHERE text = '一次新詞'")).rows.length, 0);
  await send(10, '一次新詞', 2);
  const eligible = await call(adminWords, { cookie });
  assert.equal(eligible.body.words.find(w => w.text === '一次新詞').uses, 2);
  assert.equal(eligible.body.counts.pending, before.body.counts.pending + 4);
  assert.equal((await call(adminWords, { method: 'POST', cookie, body: { text: '一次新詞', status: 'approved' } })).status, 204);
  const approved = await call(adminWords, { cookie, query: { status: 'approved' } });
  assert.equal(approved.body.words.some(w => w.text === '一次新詞'), true);
  assert.equal(approved.body.counts.approved, 1);
  assert.equal((await call(adminWords, { method: 'POST', cookie, body: { text: '一次新詞', status: 'pending' } })).status, 204);
});

test('empty syncs are recorded, invalid uploads do not advance sync time, and admin sees syncs', async () => {
  const body = { device: device(70), app: '6.1', words: [] };
  assert.equal((await call(upload, { method: 'POST', body })).status, 204);
  const first = (await db.query('SELECT * FROM device_syncs WHERE device = $1', [device(70)])).rows[0];
  assert.equal(first.app, '6.1');
  assert.ok(first.last_synced_at);
  assert.equal((await call(upload, { method: 'POST', body: { ...body, words: [{ text: 'bad' }] } })).status, 400);
  const second = (await db.query('SELECT * FROM device_syncs WHERE device = $1', [device(70)])).rows[0];
  assert.deepEqual(second.last_synced_at, first.last_synced_at);
  const signed = await call(login, { method: 'POST', body: { email: 'ino@noky.dev', password: 'right horse' } });
  const result = await call(adminWords, { cookie: signed.headers['set-cookie'].split(';')[0] });
  assert.ok(result.body.lastSyncedAt);
  assert.ok(result.body.syncs.some(s => s.device === device(70)));
});

test('deleting a submission is authenticated and prevents hourly uploads from resurrecting it', async () => {
  const body = { device: device(71), app: '6.1', words: [{ text: '刪除測試', typed: 'ㄕㄔㄘㄕ', count: 2 }] };
  await call(upload, { method: 'POST', body });
  assert.equal((await call(adminWords, { method: 'DELETE', body: { text: '刪除測試' } })).status, 401);
  const signed = await call(login, { method: 'POST', body: { email: 'ino@noky.dev', password: 'right horse' } });
  const cookie = signed.headers['set-cookie'].split(';')[0];
  assert.equal((await call(adminWords, { method: 'DELETE', cookie, body: { text: '刪除測試' } })).status, 204);
  assert.equal((await call(upload, { method: 'POST', body })).status, 204);
  assert.equal((await db.query("SELECT * FROM submissions WHERE text = '刪除測試'")).rows.length, 0);
  assert.equal((await db.query('SELECT * FROM device_syncs WHERE device = $1', [device(71)])).rows.length, 1);
});

test('curated words require full readings, support multiple readings, export and delete independently', async () => {
  const { default: lexicon } = await import('../api/admin/lexicon.js');
  const signed = await call(login, { method: 'POST', body: { email: 'ino@noky.dev', password: 'right horse' } });
  const cookie = signed.headers['set-cookie'].split(';')[0];
  const word = { text: '貢丸湯', reading: 'ㄍㄨㄥˋ ㄨㄢˊ ㄊㄤ', frequency: 2000 };
  for (const method of ['GET', 'POST', 'DELETE']) assert.equal((await call(lexicon, { method, body: word })).status, 401);
  for (const reading of ['ㄍㄨㄊ', 'abc', 'ㄍ ㄨ', 'ㄍㄍ ㄨ ㄊ', 'ㄍ ㄨ ㄊ']) {
    assert.equal((await call(lexicon, { method: 'POST', cookie, body: { ...word, reading } })).status, 400);
  }
  assert.equal((await call(lexicon, { method: 'POST', cookie, body: word })).status, 204);
  assert.equal((await call(lexicon, { method: 'POST', cookie, body: { ...word, frequency: 2500 } })).status, 204);
  const alternate = { ...word, reading: 'ㄍㄨㄥ ㄨㄢˊ ㄊㄤ' };
  await call(lexicon, { method: 'POST', cookie, body: alternate });
  const list = await call(lexicon, { cookie });
  assert.equal(list.body.words.length, 2);
  assert.equal(list.body.words.find(w => w.reading === word.reading).frequency, 2500);
  assert.equal((await call(exportWords, { cookie })).body.lexicon.length, 2);
  assert.equal((await call(lexicon, { method: 'DELETE', cookie, body: alternate })).status, 204);
  assert.deepEqual((await call(exportWords, { cookie })).body.lexicon, [{ ...word, frequency: 2500 }]);
});
