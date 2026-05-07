import { describe, it, expect, afterEach } from 'vitest';
import { makeTestApp } from './helpers.js';

describe('GET /auth/verify', () => {
  let cleanup: (() => Promise<void>) | null = null;
  afterEach(async () => { if (cleanup) await cleanup(); cleanup = null; });

  it('exchanges valid token for session cookie + creates user', async () => {
    const ctx = await makeTestApp();
    cleanup = ctx.cleanup;
    await ctx.app.inject({ method: 'POST', url: '/auth/request', headers: { 'content-type': 'application/json' }, payload: { email: 'frk@x.com' } });
    const link = ctx.mailer.outbox[0]!.text.match(/token=([\w-]+)/)![1];
    const res = await ctx.app.inject({ method: 'GET', url: `/auth/verify?token=${link}` });
    expect(res.statusCode).toBe(302);
    expect(res.headers['set-cookie']).toMatch(/rpow_session=/);
    const { rows } = await ctx.pool.query('SELECT email FROM users');
    expect(rows[0]!.email).toBe('frk@x.com');
  });

  it('rejects an unknown token', async () => {
    const ctx = await makeTestApp();
    cleanup = ctx.cleanup;
    const res = await ctx.app.inject({ method: 'GET', url: '/auth/verify?token=nope' });
    expect(res.statusCode).toBe(400);
  });

  it('rejects a reused token', async () => {
    const ctx = await makeTestApp();
    cleanup = ctx.cleanup;
    await ctx.app.inject({ method: 'POST', url: '/auth/request', headers: { 'content-type': 'application/json' }, payload: { email: 'a@b.com' } });
    const link = ctx.mailer.outbox[0]!.text.match(/token=([\w-]+)/)![1];
    await ctx.app.inject({ method: 'GET', url: `/auth/verify?token=${link}` });
    const res2 = await ctx.app.inject({ method: 'GET', url: `/auth/verify?token=${link}` });
    expect(res2.statusCode).toBe(400);
  });
});
