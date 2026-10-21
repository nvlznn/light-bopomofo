// Shared by the API functions (files starting with _ are not routes).
//
// Words users chose that the lexicon lacks, sent by the app when 分享新詞 is
// on, kept for review: the admin page (ino@noky.dev) marks each 加入 or
// 不要, and scripts/approved-words.py adds the approved ones to
// lexicon-adjustments.csv for the next release.
//
// Environment: DATABASE_URL (Neon, from the Vercel integration),
// ADMIN_PASSWORD_HASH ("scrypt:salt:hash", hex; website/scripts/admin-password.mjs),
// SESSION_SECRET (random hex).

import { createHmac, scryptSync, timingSafeEqual } from 'node:crypto';

export const ADMIN_EMAIL = 'ino@noky.dev';
const SESSION_DAYS = 7;
const COOKIE = 'lb_admin';

// ---------- Database ----------

const SCHEMA = [
  `CREATE TABLE IF NOT EXISTS device_syncs (
     device text PRIMARY KEY, app text, last_synced_at timestamptz NOT NULL DEFAULT now())`,
  `CREATE TABLE IF NOT EXISTS lexicon_entries (
     text text NOT NULL, reading text NOT NULL, frequency integer NOT NULL DEFAULT 2000,
     updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (text, reading))`,
  `CREATE TABLE IF NOT EXISTS deleted_words (text text PRIMARY KEY)`,
  `CREATE TABLE IF NOT EXISTS submissions (
     device text NOT NULL,
     text text NOT NULL,
     typed text NOT NULL,
     count integer NOT NULL,
     app text,
     first_seen timestamptz NOT NULL DEFAULT now(),
     last_seen timestamptz NOT NULL DEFAULT now(),
     PRIMARY KEY (device, text, typed))`,
  `CREATE TABLE IF NOT EXISTS decisions (
     text text PRIMARY KEY,
     status text NOT NULL CHECK (status IN ('approved', 'rejected')),
     decided_at timestamptz NOT NULL DEFAULT now())`,
];

let database = null;
let ready = null;

/** Use `query(text, params) -> rows` instead of Neon (tests). */
export function useDatabase(query) {
  database = query;
  ready = null;
}

async function neonQuery() {
  if (!process.env.DATABASE_URL) throw new HttpError(503, 'database not configured');
  const { neon } = await import('@neondatabase/serverless');
  const sql = neon(process.env.DATABASE_URL);
  return (text, params) => sql.query(text, params);
}

/** Rows of a query, with the tables created on first use. */
export async function query(text, params = []) {
  database ??= await neonQuery();
  ready ??= (async () => {
    for (const statement of SCHEMA) await database(statement, []);
  })();
  await ready;
  return database(text, params);
}

// ---------- Requests ----------

export class HttpError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

/** Run `fn(req, res)`, answering errors as JSON. */
export function handler(methods, fn) {
  return async (req, res) => {
    try {
      if (!methods.includes(req.method)) throw new HttpError(405, 'method not allowed');
      res.setHeader('Cache-Control', 'no-store');
      await fn(req, res);
    } catch (e) {
      const status = e instanceof HttpError ? e.status : 500;
      if (status === 500) console.error(e);
      res.status(status).json({ error: status === 500 ? 'server error' : e.message });
    }
  };
}

// ---------- Admin session ----------

/** "scrypt:salt:hash" for a password (hex). */
export function hashPassword(password, salt) {
  return `scrypt:${salt}:${scryptSync(password, salt, 32).toString('hex')}`;
}

export function checkLogin(email, password) {
  const stored = process.env.ADMIN_PASSWORD_HASH || '';
  const [kind, salt, hash] = stored.split(':');
  if (kind !== 'scrypt' || !salt || !hash) throw new HttpError(503, 'admin login not configured');
  const given = Buffer.from(hashPassword(String(password ?? ''), salt).split(':')[2], 'hex');
  const okPassword = timingSafeEqual(given, Buffer.from(hash, 'hex'));
  const okEmail = String(email ?? '').trim().toLowerCase() === ADMIN_EMAIL;
  return okPassword && okEmail;
}

function sign(value) {
  const secret = process.env.SESSION_SECRET;
  if (!secret) throw new HttpError(503, 'admin login not configured');
  return createHmac('sha256', secret).update(value).digest('base64url');
}

export function startSession(res) {
  const expires = String(Date.now() + SESSION_DAYS * 86400e3);
  res.setHeader('Set-Cookie',
    `${COOKIE}=${expires}.${sign(expires)}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=${SESSION_DAYS * 86400}`);
}

export function endSession(res) {
  res.setHeader('Set-Cookie', `${COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0`);
}

/** Throws 401 unless the request carries a valid admin session. */
export function requireAdmin(req) {
  const cookie = (req.headers.cookie || '').split(/;\s*/).find((c) => c.startsWith(`${COOKIE}=`));
  const [expires, signature] = (cookie || '').slice(COOKIE.length + 1).split('.');
  const valid = expires && signature && Number(expires) > Date.now()
    && signature.length === sign(expires).length
    && timingSafeEqual(Buffer.from(signature), Buffer.from(sign(expires)));
  if (!valid) throw new HttpError(401, 'not signed in');
}

// ---------- Words ----------

const HAN = /^\p{Script=Han}{1,20}$/u;
const BOPOMOFO = /^[ㄅ-ㄩˉˊˇˋ˙]{1,60}$/u;
const DEVICE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export const MAX_WORDS = 500;

/** The words of an upload, checked; throws 400 on anything else. */
export function readUpload(body) {
  const { device, app, words } = body ?? {};
  if (!DEVICE.test(String(device ?? '').toLowerCase())) throw new HttpError(400, 'bad device');
  if (!Array.isArray(words) || words.length > MAX_WORDS) throw new HttpError(400, 'bad words');
  const clean = [];
  for (const w of words) {
    const text = String(w?.text ?? '');
    const typed = String(w?.typed ?? '');
    const count = Number(w?.count);
    if (!HAN.test(text) || !BOPOMOFO.test(typed) || !Number.isInteger(count) || count < 1 || count > 1e6) {
      throw new HttpError(400, 'bad word');
    }
    clean.push({ text, typed, count });
  }
  return { device: String(device).toLowerCase(), app: String(app ?? '').slice(0, 20), words: clean };
}
