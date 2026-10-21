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
  vm.runInNewContext(html.match(/<script>([\s\S]*?)<\/script>/)[1], { document, window: { confirm: () => true, localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) } }, fetch, console });
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
  assert.match(before[0].textContent, /已核准/);
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
  assert.match(row.textContent, /已核准/);
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

test('review shows server sync time and per-word receipt time, including empty uploads', async () => {
  const p = await page();
  p.document.getElementById('refresh').click();
  p.requests[1].resolve(response({ ...data(), lastSyncedAt: '2026-10-21T03:00:00Z', syncs: [{ device: 'device-1', app: '6.1', last_synced_at: '2026-10-21T03:00:00Z' }] }));
  await tick();
  assert.match(p.document.getElementById('sync-summary').textContent, /最近成功同步/);
  assert.match(p.document.getElementById('sync-devices').textContent, /device-1.*6\.1/);
  assert.match(p.rows()[0].textContent, /2026/);
});

test('delete removes just the selected submission after server success', async () => {
  const p = await page();
  const remove = [...p.rows()[0].querySelectorAll('button')].find(b => b.textContent === '刪除');
  remove.click();
  assert.equal(p.requests[1].options.method, 'DELETE');
  assert.deepEqual(JSON.parse(p.requests[1].options.body), { text: words[0].text });
  assert.equal(p.rows().length, 3, 'keep row until deletion succeeds');
  p.requests[1].resolve(response(null, 204));
  await tick();
  assert.equal(p.rows().length, 2);
  assert.equal(p.rows()[0].querySelector('.word').textContent, words[1].text);
});

test('approved word requires a successful lookup before adding, then can be deleted', async () => {
  const p = await page();
  p.key('a'); p.requests[1].resolve(response(null, 204)); await tick();
  [...p.rows()[0].querySelectorAll('button')].find(b => b.textContent === '加入詞庫').click();
  assert.equal(p.document.getElementById('dictionary-text').value, words[0].text);
  assert.equal(p.document.getElementById('dictionary-save').disabled, true);
  assert.match(p.requests[2].url, /lexicon\?text=/);
  p.requests[2].resolve(response({ text: words[0].text, exists: false, matches: [] })); await tick();
  p.document.getElementById('dictionary-reading').value = 'ㄍㄨㄥˋ ㄨㄢˊ ㄊㄤ';
  p.document.getElementById('dictionary-form').onsubmit({ preventDefault() {} });
  const word = { text: words[0].text, reading: 'ㄍㄨㄥˋ ㄨㄢˊ ㄊㄤ', frequency: 2000 };
  assert.deepEqual(JSON.parse(p.requests[3].options.body), word);
  p.requests[3].resolve(response(null, 204)); await tick();
  assert.equal(p.requests[4].url, '/api/admin/lexicon');
  p.requests[4].resolve(response({ words: [word] })); await tick();
  assert.match(p.document.getElementById('dictionary-message').textContent, /已加入正式詞庫/);
  p.document.querySelector('#dictionary-words button').click();
  assert.equal(p.requests[5].options.method, 'DELETE');
  p.requests[5].resolve(response(null, 204)); await tick();
  assert.equal(p.document.getElementById('dictionary-words').children.length, 0);
});

function chooseAll(p) {
  const box = p.document.getElementById('select-all'); box.checked = true; box.onchange();
}

test('lookup blocks existing words, fails closed, and changing text requires another lookup', async () => {
  const p = await page(), input = p.document.getElementById('dictionary-text');
  input.value = '你好'; input.oninput();
  p.document.getElementById('dictionary-lookup').click();
  p.requests[1].resolve(response({ exists: true, matches: [{ source: 'builtin', reading: 'ㄋㄧˇ ㄏㄠˇ' }] })); await tick();
  assert.equal(p.document.getElementById('dictionary-save').disabled, true);
  assert.match(p.document.getElementById('dictionary-message').textContent, /內建詞庫/);
  input.value = '新詞測試'; input.oninput(); p.document.getElementById('dictionary-lookup').click();
  p.requests[2].resolve(response({ error: 'offline' }, 503)); await tick();
  assert.equal(p.document.getElementById('dictionary-save').disabled, true);
  p.document.getElementById('dictionary-lookup').click();
  p.requests[3].resolve(response({ exists: false, matches: [] })); await tick();
  assert.equal(p.document.getElementById('dictionary-save').disabled, false);
  input.value = '其他詞'; input.oninput();
  assert.equal(p.document.getElementById('dictionary-save').disabled, true);
  p.document.getElementById('dictionary-form').onsubmit({ preventDefault() {} });
  assert.equal(p.requests.length, 4);
});

test('batch review sends selected words once, retains selection on failure, and clears it on reload', async () => {
  const p = await page(); chooseAll(p);
  assert.match(p.document.getElementById('selected-count').textContent, /3/);
  p.document.querySelector('[data-action="approved"]').click();
  assert.deepEqual(JSON.parse(p.requests[1].options.body), { action: 'approved', words: words.map(w => w.text) });
  p.key('a'); assert.equal(p.requests.length, 2, 'shortcuts disabled during batch');
  p.requests[1].resolve(response({ error: 'offline' }, 503)); await tick();
  assert.equal(p.rows().filter(r => r.querySelector('.word-check').checked).length, 3);
  p.document.querySelector('[data-action="approved"]').click();
  p.requests[2].resolve(response(null, 204)); await tick();
  p.requests[3].resolve(response(data([]))); await tick();
  assert.match(p.document.getElementById('selected-count').textContent, /0/);
});

test('batch dictionary preparation excludes existing words and submits only reviewed missing words', async () => {
  const p = await page();
  p.document.querySelector('[data-status="approved"]').click();
  p.requests[1].resolve(response({ ...data(words.map(w => ({ ...w, status: 'approved' }))), status: 'approved' })); await tick();
  chooseAll(p); p.document.querySelector('[data-action="prepare"]').click();
  assert.equal(JSON.parse(p.requests[2].options.body).action, 'lexicon-check');
  p.requests[2].resolve(response({ words: words.map((w, i) => ({ text: w.text, matches: i ? [] : [{ reading: 'ㄍㄨㄥˋ ㄨㄢˊ ㄊㄤ', source: 'builtin' }] })) })); await tick();
  const inputs = [...p.document.querySelectorAll('#bulk-dictionary-rows input')];
  assert.equal(inputs.length, 2);
  inputs[0].value = 'ㄨㄛˇ ㄒㄧㄤˇ ㄔ ㄈㄢˋ'; inputs[1].value = 'ㄔㄣˊ ㄒㄧㄠˇ ㄇㄟˇ';
  p.document.getElementById('bulk-dictionary-form').onsubmit({ preventDefault() {} });
  assert.deepEqual(JSON.parse(p.requests[3].options.body).words.map(w => w.text), words.slice(1).map(w => w.text));
  p.requests[3].resolve(response(null, 204)); await tick();
  assert.match(p.document.getElementById('bulk-dictionary-message').textContent, /已加入 2/);
  assert.equal(p.document.getElementById('bulk-dictionary-save').disabled, true);
});

test('dictionary batch deletion keeps rows on error and deletes selected rows only on success', async () => {
  const p = await page();
  const items = [{ text: '詞甲', reading: 'ㄘˊ ㄐㄧㄚˇ', frequency: 2000 }, { text: '詞乙', reading: 'ㄘˊ ㄧˇ', frequency: 2000 }];
  p.document.getElementById('dictionary-refresh').click();
  p.requests[1].resolve(response({ words: items })); await tick();
  const box = p.document.querySelector('.dictionary-check'); box.checked = true; box.onchange();
  p.document.getElementById('dictionary-delete-selected').click();
  assert.deepEqual(JSON.parse(p.requests[2].options.body), { action: 'lexicon-delete', words: [items[0]] });
  p.requests[2].resolve(response({ error: 'offline' }, 503)); await tick();
  assert.equal(p.document.querySelectorAll('.dictionary-check').length, 2);
  assert.equal(box.checked, true);
  p.document.getElementById('dictionary-delete-selected').click();
  p.requests[3].resolve(response(null, 204)); await tick();
  assert.equal(p.document.querySelectorAll('.dictionary-check').length, 1);
  assert.match(p.document.getElementById('dictionary-words').textContent, /詞乙/);
});

test('switching review tabs clears checked words so hidden selections cannot be processed', async () => {
  const p = await page(); chooseAll(p);
  p.document.querySelector('[data-status="rejected"]').click();
  p.requests[1].resolve(response({ words: [{ ...words[0], status: 'rejected' }], counts: { rejected: 1 } })); await tick();
  assert.match(p.document.getElementById('selected-count').textContent, /0/);
  assert.equal(p.document.querySelector('[data-action="delete"]').disabled, true);
});
