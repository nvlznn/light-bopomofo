// POST /api/admin/login {"email", "password"} — sets the session cookie.

import { checkLogin, handler, HttpError, startSession } from '../_lib.js';

export default handler(['POST'], async (req, res) => {
  const { email, password } = req.body ?? {};
  if (!checkLogin(email, password)) throw new HttpError(401, '帳號或密碼不對');
  startSession(res);
  res.status(204).end();
});
