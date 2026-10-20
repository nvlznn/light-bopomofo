// The admin page's password hash (ADMIN_PASSWORD_HASH) for a password, or a
// new random password and its hash:
//   node scripts/admin-password.mjs ["password"]

import { randomBytes } from 'node:crypto';

import { hashPassword } from '../api/_lib.js';

const password = process.argv[2] ?? randomBytes(15).toString('base64url');
console.log(`password\t${password}`);
console.log(`ADMIN_PASSWORD_HASH\t${hashPassword(password, randomBytes(16).toString('hex'))}`);
