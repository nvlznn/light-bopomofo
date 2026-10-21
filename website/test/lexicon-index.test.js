import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

test('admin builtin lookup stays aligned with shipped lexicon adjustments', async () => {
  const index = JSON.parse(await readFile(new URL('../api/_data/lexicon.json', import.meta.url), 'utf8'));
  const adjustments = await readFile(new URL('../../crates/lightbopomofo-core/lexicon-adjustments.csv', import.meta.url), 'utf8');
  assert.equal(index.adjustmentsSha256, createHash('sha256').update(adjustments).digest('hex'), 'Regenerate with python3 scripts/gen-admin-lexicon.py before publishing');
  for (const line of adjustments.split('\n')) {
    if (line.startsWith('-')) assert.equal(Object.hasOwn(index.words, line.slice(1).trim()), false);
  }
  assert.ok(index.words['你好'].length);
});
