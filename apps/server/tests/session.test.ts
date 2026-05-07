import { describe, it, expect } from 'vitest';
import { signSession, verifySession } from '../src/session.js';

describe('session token', () => {
  const secret = 's'.repeat(32);
  it('signed token verifies and yields email', () => {
    const tok = signSession({ email: 'a@b.com' }, secret, 60);
    const claim = verifySession(tok, secret);
    expect(claim?.email).toBe('a@b.com');
  });
  it('expired tokens fail', () => {
    const tok = signSession({ email: 'a@b.com' }, secret, -1);
    expect(verifySession(tok, secret)).toBeNull();
  });
  it('tampered tokens fail', () => {
    const tok = signSession({ email: 'a@b.com' }, secret, 60);
    expect(verifySession(tok + 'x', secret)).toBeNull();
  });
});
