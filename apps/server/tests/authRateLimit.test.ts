import { describe, it, expect, afterEach } from 'vitest';
import { makeTestApp } from './helpers.js';

describe('magic-link rate limiting', () => {
  let cleanup: (() => Promise<void>) | null = null;
  afterEach(async () => { if (cleanup) await cleanup(); cleanup = null; });

  it('cools down a 2nd request to same email within 30s', async () => {
    const ctx = await makeTestApp(); cleanup = ctx.cleanup;
    const ok = await ctx.app.inject({ method: 'POST', url: '/auth/request', payload: { email: 'a@b.com' }, headers: { 'content-type': 'application/json' } });
    expect(ok.statusCode).toBe(200);
    const limited = await ctx.app.inject({ method: 'POST', url: '/auth/request', payload: { email: 'a@b.com' }, headers: { 'content-type': 'application/json' } });
    expect(limited.statusCode).toBe(429);
    const body = limited.json();
    expect(body.error).toBe('RATE_LIMITED');
    expect(body.retry_after).toBeGreaterThan(0);
  });
});
