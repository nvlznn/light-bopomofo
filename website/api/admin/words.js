// GET  /api/admin/words?status=pending|approved|rejected — the words, by how
//      many devices sent them; filtered by minUses (default 2).
// POST /api/admin/words {"text", "status": "approved"|"rejected"|"pending"}
//      — decide on a word (pending undoes a decision).

import { handler, HttpError, query, requireAdmin } from '../_lib.js';

const STATUSES = ['pending', 'approved', 'rejected'];

export default handler(['GET', 'POST', 'DELETE'], async (req, res) => {
  requireAdmin(req);
  if (req.method === 'DELETE') {
    const { text } = req.body ?? {};
    if (typeof text !== 'string' || !/^\p{Script=Han}{1,20}$/u.test(text)) throw new HttpError(400, 'bad word');
    await query(`WITH blocked AS (
      INSERT INTO deleted_words (text) VALUES ($1) ON CONFLICT DO NOTHING
    ), removed AS (DELETE FROM submissions WHERE text = $1)
    DELETE FROM decisions WHERE text = $1`, [text]);
    res.status(204).end();
    return;
  }
  if (req.method === 'POST') {
    const { text, status } = req.body ?? {};
    if (typeof text !== 'string' || !STATUSES.includes(status)) throw new HttpError(400, 'bad decision');
    if (status === 'pending') {
      await query('DELETE FROM decisions WHERE text = $1', [text]);
    } else {
      const saved = await query(
        `INSERT INTO decisions (text, status)
         SELECT $1, $2 FROM submissions WHERE text = $1
         HAVING coalesce(sum(count), 0) >= 1
         ON CONFLICT (text) DO UPDATE SET status = EXCLUDED.status, decided_at = now()
         RETURNING text`,
        [text, status],
      );
      if (saved.length === 0) throw new HttpError(400, '這個詞尚未有輸入紀錄');
    }
    res.status(204).end();
    return;
  }
  const status = STATUSES.includes(req.query?.status) ? req.query.status : 'pending';
  const rawMinimum = req.query?.minUses ?? '2';
  const minUses = Number(rawMinimum);
  if (!/^\d+$/.test(String(rawMinimum)) || !Number.isSafeInteger(minUses) || minUses < 1 || minUses > 2147483647) {
    throw new HttpError(400, '最低次數必須是 1 到 2147483647 的整數');
  }
  const rows = await query(
    `SELECT s.text,
            array_agg(DISTINCT s.typed) AS typed,
            count(DISTINCT s.device)::int AS devices,
            sum(s.count)::int AS uses,
            max(s.last_seen) AS last_seen,
            coalesce(d.status, 'pending') AS status
     FROM submissions s LEFT JOIN decisions d ON d.text = s.text
     WHERE coalesce(d.status, 'pending') = $1
     GROUP BY s.text, d.status
     HAVING sum(s.count) >= $2
     ORDER BY devices DESC, uses DESC, last_seen DESC, s.text ASC
     LIMIT 1000`,
    [status, minUses],
  );
  const counts = await query(
    `SELECT status, count(*)::int AS words
     FROM (
       SELECT s.text, coalesce(d.status, 'pending') AS status
       FROM submissions s LEFT JOIN decisions d ON d.text = s.text
       GROUP BY s.text, d.status
       HAVING sum(s.count) >= $1
     ) eligible
     GROUP BY status`,
    [minUses],
  );
  const syncs = await query('SELECT device, app, last_synced_at FROM device_syncs ORDER BY last_synced_at DESC LIMIT 20');
  res.status(200).json({ status, minUses, syncs, lastSyncedAt: syncs[0]?.last_synced_at ?? null, words: rows, counts: Object.fromEntries(counts.map((c) => [c.status, c.words])) });
});
