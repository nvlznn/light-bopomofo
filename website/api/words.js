// POST /api/words — the app's new words (分享新詞):
//   {"device": "<random uuid>", "app": "3.0", "words": [{"text", "typed", "count"}]}
// Each device's count for a word and its symbols keeps the highest seen.

import { handler, query, readUpload } from './_lib.js';

export default handler(['POST'], async (req, res) => {
  const { device, app, words } = readUpload(req.body);
  if (words.length > 0) {
    const values = [];
    const params = [];
    for (const w of words) {
      const i = params.length;
      values.push(`($${i + 1}, $${i + 2}, $${i + 3}, $${i + 4}, $${i + 5})`);
      params.push(device, w.text, w.typed, w.count, app);
    }
    await query(
      `INSERT INTO submissions (device, text, typed, count, app)
       SELECT v.device, v.text, v.typed, v.count::integer, v.app
       FROM (VALUES ${values.join(', ')}) AS v(device, text, typed, count, app)
       WHERE NOT EXISTS (SELECT 1 FROM deleted_words d WHERE d.text = v.text)
       ON CONFLICT (device, text, typed) DO UPDATE
       SET count = GREATEST(submissions.count, EXCLUDED.count), app = EXCLUDED.app, last_seen = now()`,
      params,
    );
  }
  await query(`INSERT INTO device_syncs (device, app) VALUES ($1, $2)
    ON CONFLICT (device) DO UPDATE SET app = EXCLUDED.app, last_synced_at = now()`, [device, app]);
  res.status(204).end();
});
