// The curated dictionary. Full readings are entered by the reviewer, not guessed from initials.
import { handler, HttpError, query, requireAdmin } from '../_lib.js';

export default handler(['GET', 'POST', 'DELETE'], async (req, res) => {
  requireAdmin(req);
  if (req.method === 'GET') {
    const words = await query('SELECT text, reading, frequency, updated_at FROM lexicon_entries ORDER BY updated_at DESC, text, reading');
    res.status(200).json({ words });
    return;
  }
  const { text, reading, frequency = 2000 } = req.body ?? {};
  if (typeof text !== 'string' || !/^\p{Script=Han}{1,20}$/u.test(text) || typeof reading !== 'string') {
    throw new HttpError(400, '請填寫詞與完整注音');
  }
  const syllables = reading.trim().split(/\s+/);
  if (syllables.length !== [...text].length || !syllables.every(s => /^[ㄅ-ㄙ]?[ㄧㄨㄩ]?[ㄚ-ㄦ]?[ˉˊˇˋ˙]?$/.test(s) && (/[ㄧㄨㄩㄚ-ㄦ]/.test(s) || /^[ㄓ-ㄙ][ˉˊˇˋ˙]?$/.test(s)))) {
    throw new HttpError(400, '請填寫每個字的完整注音，以空格分隔');
  }
  const normalized = syllables.join(' ');
  if (req.method === 'DELETE') {
    await query('DELETE FROM lexicon_entries WHERE text = $1 AND reading = $2', [text, normalized]);
  } else {
    if (!Number.isInteger(frequency) || frequency < 1 || frequency > 1000000) throw new HttpError(400, '詞頻必須是 1 到 1000000 的整數');
    await query(`INSERT INTO lexicon_entries (text, reading, frequency) VALUES ($1, $2, $3)
      ON CONFLICT (text, reading) DO UPDATE SET frequency = EXCLUDED.frequency, updated_at = now()`, [text, normalized, frequency]);
  }
  res.status(204).end();
});
