import { createHmac, timingSafeEqual } from 'node:crypto';

export interface SessionClaim { email: string; exp: number }

export function signSession(claim: { email: string }, secret: string, ttlSeconds: number): string {
  const exp = Math.floor(Date.now() / 1000) + ttlSeconds;
  const body = Buffer.from(JSON.stringify({ email: claim.email, exp })).toString('base64url');
  const sig = createHmac('sha256', secret).update(body).digest('base64url');
  return `${body}.${sig}`;
}

export function verifySession(token: string, secret: string): SessionClaim | null {
  const dot = token.indexOf('.');
  if (dot < 0) return null;
  const body = token.slice(0, dot);
  const sig = token.slice(dot + 1);
  const expected = createHmac('sha256', secret).update(body).digest('base64url');
  const a = Buffer.from(sig); const b = Buffer.from(expected);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return null;
  try {
    const c = JSON.parse(Buffer.from(body, 'base64url').toString('utf8')) as SessionClaim;
    if (typeof c.email !== 'string' || typeof c.exp !== 'number') return null;
    if (Math.floor(Date.now() / 1000) >= c.exp) return null;
    return c;
  } catch { return null; }
}

export const SESSION_COOKIE = 'rpow_session';
export const SESSION_TTL_SECONDS = 60 * 60 * 24 * 30;
