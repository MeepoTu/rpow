import { describe, it, expect } from 'vitest';
import { issueMagicLink, verifyMagicLink, hashToken } from '../src/magic.js';

describe('magic-link primitives', () => {
  it('issued token verifies', () => {
    const { token, hash } = issueMagicLink();
    expect(hashToken(token).equals(hash)).toBe(true);
  });
  it('verifyMagicLink returns true on match', () => {
    const { token, hash } = issueMagicLink();
    expect(verifyMagicLink(token, hash)).toBe(true);
  });
  it('verifyMagicLink returns false on mismatch', () => {
    const { hash } = issueMagicLink();
    expect(verifyMagicLink('wrong', hash)).toBe(false);
  });
});
