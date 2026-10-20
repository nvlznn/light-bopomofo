// GET  /api/admin/words?status=pending|approved|rejected — the words, by how
//      many devices sent them.
// POST /api/admin/words {"text", "status": "approved"|"rejected"|"pending"}
//      — decide on a word (pending undoes a decision).

import { handler, HttpError, query, requireAdmin } from '../_lib.js';

const STATUSES = ['pending', 'approved', 'rejected'];

export default handler(['GET', 'POST'], async (req, res) => {
  requireAdmin(req);
  if (req.method === 'POST') {
    const { text, status } = req.body ?? {};
    if (typeof text !== 'string' || !STATUSES.includes(status)) throw new HttpError(400, 'bad decision');
    if (status === 'pending') {
      await query('DELETE FROM decisions WHERE text = $1', [text]);
    } else {
      await query(
        `INSERT INTO decisions (text, status) VALUES ($1, $2)
         ON CONFLICT (text) DO UPDATE SET status = EXCLUDED.status, decided_at = now()`,
        [text, status],
      );
    }
    res.status(204).end();
    return;
  }
  const status = STATUSES.includes(req.query?.status) ? req.query.status : 'pending';
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
     ORDER BY devices DESC, uses DESC, last_seen DESC
     LIMIT 1000`,
    [status],
  );
  const counts = await query(
    `SELECT coalesce(d.status, 'pending') AS status, count(DISTINCT s.text)::int AS words
     FROM submissions s LEFT JOIN decisions d ON d.text = s.text
     GROUP BY 1`,
  );
  res.status(200).json({ status, words: rows, counts: Object.fromEntries(counts.map((c) => [c.status, c.words])) });
});
