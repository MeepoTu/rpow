import { describe, it, expect, afterEach } from 'vitest';
import { makeTestApp } from './helpers.js';

async function login(ctx: Awaited<ReturnType<typeof makeTestApp>>, email = 'a@b.com'): Promise<string> {
  await ctx.app.inject({ method: 'POST', url: '/auth/request', payload: { email }, headers: { 'content-type': 'application/json' } });
  const tok = ctx.mailer.outbox.at(-1)!.text.match(/token=([\w-]+)/)![1];
  const r = await ctx.app.inject({ method: 'GET', url: `/auth/verify?token=${tok}` });
  return r.headers['set-cookie'] as string;
}

describe('POST /challenge', () => {
  let cleanup: (() => Promise<void>) | null = null;
  afterEach(async () => { if (cleanup) await cleanup(); cleanup = null; });

  it('issues a challenge to a logged-in user', async () => {
    const ctx = await makeTestApp(); cleanup = ctx.cleanup;
    const cookie = await login(ctx);
    const res = await ctx.app.inject({ method: 'POST', url: '/challenge', headers: { cookie } });
    expect(res.statusCode).toBe(200);
    const body = res.json();
    expect(body.challenge_id).toMatch(/^[0-9a-f-]{36}$/);
    expect(body.nonce_prefix).toMatch(/^[0-9a-f]+$/);
    expect(body.difficulty_bits).toBe(8);
  });

  it('rejects unauthenticated', async () => {
    const ctx = await makeTestApp(); cleanup = ctx.cleanup;
    const res = await ctx.app.inject({ method: 'POST', url: '/challenge' });
    expect(res.statusCode).toBe(401);
  });

  async function seedRootTokens(ctx: Awaited<ReturnType<typeof makeTestApp>>, n: number) {
    const { randomUUID } = await import('node:crypto');
    for (let i = 0; i < n; i++) {
      await ctx.pool.query(
        `INSERT INTO tokens(id, owner_email, value, state, server_sig)
         VALUES ($1, $2, 1, 'VALID', '\\x00')`,
        [randomUUID(), `seed-${i}@x.com`],
      );
    }
  }

  it('stamps base difficulty (8) below first milestone', async () => {
    const ctx = await makeTestApp(); cleanup = ctx.cleanup;
    const cookie = await login(ctx);
    await seedRootTokens(ctx, 5); // supply = 5, epoch 0
    const body = (await ctx.app.inject({ method: 'POST', url: '/challenge', headers: { cookie } })).json();
    expect(body.difficulty_bits).toBe(8);
  });

  it('stamps +1 bit (9) past first milestone', async () => {
    const ctx = await makeTestApp(); cleanup = ctx.cleanup;
    const cookie = await login(ctx);
    await seedRootTokens(ctx, 10); // supply = 10, epoch 1
    const body = (await ctx.app.inject({ method: 'POST', url: '/challenge', headers: { cookie } })).json();
    expect(body.difficulty_bits).toBe(9);
  });

  it('refuses with 410 SUPPLY_EXHAUSTED at cap', async () => {
    const ctx = await makeTestApp(); cleanup = ctx.cleanup;
    const cookie = await login(ctx);
    await seedRootTokens(ctx, 21); // supply = 21, at cap
    const res = await ctx.app.inject({ method: 'POST', url: '/challenge', headers: { cookie } });
    expect(res.statusCode).toBe(410);
    expect(res.json().error).toBe('SUPPLY_EXHAUSTED');
  });
});
