import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import vm from 'node:vm';
import { parseHTML } from 'linkedom';

const html = await readFile(new URL('../admin.html', import.meta.url), 'utf8');
const tick = () => new Promise(resolve => setImmediate(resolve));
const response = (body, status = 200) => ({ status, ok: status < 400, json: async () => body });
const words = ['貢丸湯', '我想吃飯', '陳小美'].map(text => ({ text, typed: ['ㄍ'], devices: 1, uses: 2, last_seen: '2026-10-20', status: 'pending' }));
const data = (entries = words) => ({ words: entries, counts: { pending: 3 }, status: 'pending' });

async function page(storage = new Map()) {
  const { document, window } = parseHTML(html);
  const requests = [];
  const fetch = (url, options = {}) => new Promise(resolve => requests.push({ url, options, resolve }));
  vm.runInNewContext(html.match(/<script>([\s\S]*?)<\/script>/)[1], { document, window: { localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) } }, fetch, console });
  requests[0].resolve(response(data()));
  await tick();
  const key = (key, extra = {}) => {
    const event = new window.Event('keydown', { bubbles: true, cancelable: true });
    Object.assign(event, { key, ...extra });
    document.dispatchEvent(event);
    return event;
  };
  return { document, window, requests, key, rows: () => [...document.querySelectorAll('#rows tr')] };
}

test('saving retains every row and its position without refetching the list', async () => {
  const p = await page();
  const before = p.rows();
  before[0].querySelector('.ok').click();
  assert.equal(p.requests.length, 2);
  p.requests[1].resolve(response(null, 204));
  await tick();
  assert.equal(p.requests.length, 2, 'a decision must not trigger another list request');
  assert.deepEqual(p.rows(), before, 'the same row nodes remain in the same order');
  assert.match(before[0].textContent, /已加入/);
  assert.equal(p.document.querySelector('[data-status="pending"] .n').textContent, '2');
});

test('shortcuts can review the next word while the first save is still pending', async () => {
  const p = await page();
  p.key('a');
  p.key('d');
  assert.equal(p.requests.length, 3);
  assert.deepEqual(p.requests.slice(1).map(r => JSON.parse(r.options.body)), [
    { text: words[0].text, status: 'approved' }, { text: words[1].text, status: 'rejected' },
  ]);
  assert.match(p.rows()[0].textContent, /儲存中/);
  assert.equal(p.rows()[2].getAttribute('data-selected'), 'true');
  p.requests[2].resolve(response(null, 204));
  p.requests[1].resolve(response(null, 204));
  await tick();
  assert.equal(p.document.querySelector('[data-status="pending"] .n').textContent, '1');
});

test('failed saves show an error in place and permit retry', async () => {
  const p = await page();
  const row = p.rows()[0];
  row.querySelector('.ok').click();
  p.requests[1].resolve(response({ error: 'offline' }, 503));
  await tick();
  assert.match(row.textContent, /儲存失敗/);
  assert.equal(row.querySelector('.ok').disabled, false);
  row.querySelector('.ok').click();
  assert.equal(p.requests.length, 3);
  p.requests[2].resolve(response(null, 204));
  await tick();
  assert.match(row.textContent, /已加入/);
});

test('composition, held keys and modifier shortcuts never submit a decision', async () => {
  const p = await page();
  for (const extra of [{ isComposing: true }, { repeat: true }, { metaKey: true }, { ctrlKey: true }, { altKey: true }]) p.key('a', extra);
  assert.equal(p.requests.length, 1);
  p.key('ArrowDown');
  p.key('a');
  assert.equal(JSON.parse(p.requests[1].options.body).text, words[1].text);
});

test('undo returns a saved word to pending without moving it', async () => {
  const p = await page();
  p.key('a');
  p.requests[1].resolve(response(null, 204));
  await tick();
  p.rows()[0].click();
  p.key('u');
  assert.equal(p.requests.length, 3);
  assert.deepEqual(JSON.parse(p.requests[2].options.body), { text: words[0].text, status: 'pending' });
  p.requests[2].resolve(response(null, 204));
  await tick();
  assert.equal(p.rows()[0].querySelector('.word').textContent, words[0].text);
  assert.equal(p.document.querySelector('[data-status="pending"] .n').textContent, '3');
});

test('late responses from an older tab cannot overwrite the current tab', async () => {
  const p = await page();
  p.document.querySelector('[data-status="approved"]').click();
  p.document.querySelector('[data-status="rejected"]').click();
  p.requests[2].resolve(response({ words: [{ ...words[2], status: 'rejected' }], counts: {}, status: 'rejected' }));
  await tick();
  p.requests[1].resolve(response({ words: [{ ...words[0], status: 'approved' }], counts: {}, status: 'approved' }));
  await tick();
  assert.equal(p.rows()[0].querySelector('.word').textContent, words[2].text);
});


test('minimum-use filter is sent to the API and remembered on reopening', async () => {
  const storage = new Map();
  const p = await page(storage);
  assert.match(p.requests[0].url, /minUses=2/);
  p.document.getElementById('min-uses').value = '5';
  p.document.getElementById('filters').onsubmit({ preventDefault() {} });
  assert.match(p.requests[1].url, /minUses=5/);
  assert.equal(storage.get('lb-review-min-uses'), '5');
  p.requests[1].resolve(response(data([])));
  await tick();
  const reopened = await page(storage);
  assert.equal(reopened.document.getElementById('min-uses').value, '5');
  assert.match(reopened.requests[0].url, /minUses=5/);
  reopened.document.getElementById('min-uses').value = '1';
  reopened.document.getElementById('filters').onsubmit({ preventDefault() {} });
  assert.match(reopened.requests[1].url, /minUses=1/);
});

test('editing the filter does not activate review shortcuts', async () => {
  const p = await page();
  const input = p.document.getElementById('min-uses');
  const event = new p.window.Event('keydown', { bubbles: true });
  event.key = 'a';
  input.dispatchEvent(event);
  assert.equal(p.requests.length, 1);
  input.value = '0';
  p.document.getElementById('filters').onsubmit({ preventDefault() {} });
  assert.equal(p.requests.length, 1);
  assert.match(p.document.getElementById('review-message').textContent, /整數/);
});

test('only the most recently reviewed word remains, regardless of response order', async () => {
  const p = await page();
  p.key('a');
  p.key('d');
  p.requests[2].resolve(response(null, 204));
  await tick();
  assert.equal(p.rows()[0].hidden, false, 'in-flight save remains visible');
  p.requests[1].resolve(response(null, 204));
  await tick();
  assert.equal(p.rows()[0].hidden, true);
  assert.equal(p.rows()[1].hidden, false);
  assert.equal(p.rows()[2].hidden, false);
  p.key('ArrowUp');
  assert.equal(p.rows()[1].getAttribute('data-selected'), 'true');
  p.key('ArrowUp');
  assert.equal(p.rows()[1].getAttribute('data-selected'), 'true', 'skip hidden rows');
});

test('U from the next pending word undoes the previous decision', async () => {
  const p = await page();
  p.key('a');
  p.requests[1].resolve(response(null, 204));
  await tick();
  p.key('u');
  assert.deepEqual(JSON.parse(p.requests[2].options.body), { text: words[0].text, status: 'pending' });
  p.requests[2].resolve(response(null, 204));
  await tick();
  assert.equal(p.rows()[0].hidden, false);
});
