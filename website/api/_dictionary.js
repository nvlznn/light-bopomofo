import { readFileSync } from 'node:fs';
import { HttpError, query } from './_lib.js';

const builtin = JSON.parse(readFileSync(new URL('./_data/lexicon.json', import.meta.url), 'utf8')).words;

export function normalizeWord(body) {
  const { text, reading, frequency = 2000 } = body ?? {};
  if (typeof text !== 'string' || !/^\p{Script=Han}{1,20}$/u.test(text) || typeof reading !== 'string') {
    throw new HttpError(400, '請填寫詞與完整注音');
  }
  const syllables = reading.trim().split(/\s+/);
  if (syllables.length !== [...text].length || !syllables.every(s => /^[ㄅ-ㄙ]?[ㄧㄨㄩ]?[ㄚ-ㄦ]?[ˉˊˇˋ˙]?$/.test(s) && (/[ㄧㄨㄩㄚ-ㄦ]/.test(s) || /^[ㄓ-ㄙ][ˉˊˇˋ˙]?$/.test(s)))) {
    throw new HttpError(400, '請填寫每個字的完整注音，以空格分隔');
  }
  const normalized = syllables.join(' ');
  if (!Number.isInteger(frequency) || frequency < 1 || frequency > 1000000) throw new HttpError(400, '詞頻必須是 1 到 1000000 的整數');
  return { text, reading: normalized, frequency };
}

export async function lookupWords(texts) {
  const rows = await query('SELECT text, reading, frequency FROM lexicon_entries WHERE text = ANY($1::text[]) ORDER BY text, reading', [texts]);
  return texts.map(text => ({ text, matches: [
    ...(Object.hasOwn(builtin, text) ? builtin[text].map(reading => ({ reading, source: 'builtin' })) : []),
    ...rows.filter(w => w.text === text).map(w => ({ reading: w.reading, frequency: w.frequency, source: 'curated' })),
  ] }));
}

export async function addWords(words) {
  if (new Set(words.map(w => w.text)).size !== words.length) throw new HttpError(400, '同一批有重複的詞');
  const existing = (await lookupWords(words.map(w => w.text))).filter(w => w.matches.length);
  if (existing.length) throw new HttpError(409, '詞已存在：' + existing.map(w => w.text).join('、'));
  try {
    // Unique term reservations make concurrent additions fail atomically.
    await query(`WITH reserved AS (
      INSERT INTO lexicon_terms (text) SELECT unnest($1::text[]) RETURNING text
    ) INSERT INTO lexicon_entries (text, reading, frequency)
    SELECT w.text, w.reading, w.frequency
    FROM jsonb_to_recordset($2::jsonb) AS w(text text, reading text, frequency integer)
    JOIN reserved r ON r.text = w.text`, [words.map(w => w.text), JSON.stringify(words)]);
  } catch (error) {
    if (error.code === '23505') throw new HttpError(409, '詞已被加入，請重新查詢；這批未儲存');
    throw error;
  }
}

export async function deleteWords(words) {
  await query(`WITH removed AS (
    DELETE FROM lexicon_entries l USING jsonb_to_recordset($1::jsonb) AS w(text text, reading text)
    WHERE l.text = w.text AND l.reading = w.reading RETURNING l.text, l.reading
  ) DELETE FROM lexicon_terms t WHERE t.text IN (SELECT text FROM removed)
  AND NOT EXISTS (SELECT 1 FROM lexicon_entries l WHERE l.text = t.text
    AND NOT EXISTS (SELECT 1 FROM removed r WHERE r.text = l.text AND r.reading = l.reading))`, [JSON.stringify(words)]);
}
