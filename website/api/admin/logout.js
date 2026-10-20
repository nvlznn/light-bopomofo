// POST /api/admin/logout — clears the session cookie.

import { endSession, handler } from '../_lib.js';

export default handler(['POST'], async (req, res) => {
  endSession(res);
  res.status(204).end();
});
