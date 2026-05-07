import { randomBytes, createHash, timingSafeEqual } from 'node:crypto';

/** 256-bit random token, base64url encoded. */
export function issueMagicLink(): { token: string; hash: Buffer } {
  const raw = randomBytes(32);
  const token = raw.toString('base64url');
  return { token, hash: hashToken(token) };
}

export function hashToken(token: string): Buffer {
  return createHash('sha256').update(token).digest();
}

export function verifyMagicLink(token: string, expectedHash: Buffer): boolean {
  const got = hashToken(token);
  return got.length === expectedHash.length && timingSafeEqual(got, expectedHash);
}
