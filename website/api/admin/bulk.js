import { handler, HttpError, query, requireAdmin } from '../_lib.js';
import { normalizeWord, addWords, deleteWords, lookupWords } from '../_dictionary.js';

export default handler(['POST'], async (req, res) => {
  requireAdmin(req);
  const { action, words } = req.body ?? {};
  if (!Array.isArray(words) || words.length < 1 || words.length > 1000) throw new HttpError(400, '每批請選擇 1 到 1000 個詞');
  if (action === 'lexicon-check') {
    if (!words.every(w => typeof w === 'string' && /^\p{Script=Han}{1,20}$/u.test(w))) throw new HttpError(400, '詞無效');
    res.status(200).json({ words: await lookupWords(words) });
    return;
  }
  if (action === 'lexicon-add' || action === 'lexicon-delete') {
    const entries = words.map(normalizeWord);
    if (action === 'lexicon-add') await addWords(entries);
    else await deleteWords(entries);
  } else {
    if (!['approved', 'rejected', 'pending', 'delete'].includes(action)) throw new HttpError(400, '不支援的批次操作');
    if (!words.every(w => typeof w === 'string' && /^\p{Script=Han}{1,20}$/u.test(w)) || new Set(words).size !== words.length) throw new HttpError(400, '詞無效或重複');
    if (action === 'delete') {
      await query(`WITH blocked AS (
        INSERT INTO deleted_words (text) SELECT unnest($1::text[]) ON CONFLICT DO NOTHING
      ), removed AS (DELETE FROM submissions WHERE text = ANY($1::text[]))
      DELETE FROM decisions WHERE text = ANY($1::text[])`, [words]);
    } else if (action === 'pending') {
      await query('DELETE FROM decisions WHERE text = ANY($1::text[])', [words]);
    } else {
      // Gate the entire INSERT on all requested words existing, not a partial subset.
      const saved = await query(`WITH eligible AS (
        SELECT DISTINCT text FROM submissions WHERE text = ANY($1::text[])
      ) INSERT INTO decisions (text, status)
      SELECT text, $2 FROM eligible WHERE (SELECT count(*) FROM eligible) = cardinality($1::text[])
      ON CONFLICT (text) DO UPDATE SET status = EXCLUDED.status, decided_at = now() RETURNING text`, [words, action]);
      if (saved.length !== words.length) throw new HttpError(409, '部分詞已不存在，請重整後再試；這批未儲存');
    }
  }
  res.status(204).end();
});
