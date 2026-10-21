import { handler, HttpError, query, requireAdmin } from '../_lib.js';
import { normalizeWord, lookupWords, addWords, deleteWords } from '../_dictionary.js';

export default handler(['GET', 'POST', 'DELETE'], async (req, res) => {
  requireAdmin(req);
  if (req.method === 'GET') {
    if (req.query?.text !== undefined) {
      const text = req.query.text;
      if (typeof text !== 'string' || !/^\p{Script=Han}{1,20}$/u.test(text)) throw new HttpError(400, '請輸入 1 到 20 個中文字');
      const [result] = await lookupWords([text]);
      res.status(200).json({ ...result, exists: result.matches.length > 0 });
    } else {
      const words = await query('SELECT text, reading, frequency, updated_at FROM lexicon_entries ORDER BY updated_at DESC, text, reading');
      res.status(200).json({ words });
    }
    return;
  }
  const word = normalizeWord(req.body);
  if (req.method === 'DELETE') await deleteWords([word]);
  else await addWords([word]);
  res.status(204).end();
});
