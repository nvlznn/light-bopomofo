// GET /api/admin/export — the approved words, each with the symbols it was
// typed with and how often (for scripts/approved-words.py).

import { handler, query, requireAdmin } from '../_lib.js';

export default handler(['GET'], async (req, res) => {
  requireAdmin(req);
  const rows = await query(
    `SELECT s.text, s.typed, sum(s.count)::int AS uses, count(DISTINCT s.device)::int AS devices
     FROM submissions s JOIN decisions d ON d.text = s.text AND d.status = 'approved'
     GROUP BY s.text, s.typed
     ORDER BY s.text, uses DESC`,
  );
  const lexicon = await query('SELECT text, reading, frequency FROM lexicon_entries ORDER BY text, reading');
  res.status(200).json({ words: rows, lexicon });
});
