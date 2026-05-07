import type { FastifyInstance } from 'fastify';
import { randomUUID } from 'node:crypto';
import { z } from 'zod';
import { issueMagicLink, verifyMagicLink } from '../magic.js';
import { signSession, SESSION_COOKIE, SESSION_TTL_SECONDS, verifySession } from '../session.js';

const RequestBody = z.object({ email: z.string().email() });

export async function authRoutes(app: FastifyInstance) {
  app.post('/auth/request', async (req, reply) => {
    const parsed = RequestBody.safeParse(req.body);
    if (!parsed.success) return reply.code(400).send({ error: 'BAD_REQUEST', message: 'invalid email' });
    const email = parsed.data.email.toLowerCase().trim();

    const { token, hash } = issueMagicLink();
    const id = randomUUID();
    const expiresAt = new Date(Date.now() + 15 * 60 * 1000);
    await app.pool.query(
      'INSERT INTO magic_links(id, email, token_hash, expires_at) VALUES($1,$2,$3,$4)',
      [id, email, hash, expiresAt],
    );
    const link = `${app.config.magicLinkBaseUrl}/auth/verify?token=${token}`;
    await app.mailer.send({
      to: email,
      subject: 'rpow2 — your magic link',
      text: `Click to sign in:\n${link}\n\nLink expires in 15 minutes.`,
      html: `<p>Click to sign in to <a href="${link}">rpow2</a>.</p><p><a href="${link}">${link}</a></p><p>Link expires in 15 minutes.</p>`,
    });
    return { ok: true, cooldown_seconds: 30 };
  });

  app.get('/auth/verify', async (req, reply) => {
    const token = (req.query as Record<string, string>).token;
    if (!token) return reply.code(400).send({ error: 'BAD_REQUEST', message: 'missing token' });

    const { rows } = await app.pool.query(
      'SELECT id, email, token_hash, expires_at, used_at FROM magic_links WHERE expires_at > now() AND used_at IS NULL',
    );
    const match = rows.find(r => verifyMagicLink(token, r.token_hash));
    if (!match) return reply.code(400).send({ error: 'BAD_REQUEST', message: 'invalid or expired link' });

    await app.pool.query('UPDATE magic_links SET used_at=now() WHERE id=$1', [match.id]);

    await app.pool.query(
      `INSERT INTO users(email) VALUES($1)
       ON CONFLICT (email) DO UPDATE SET last_login_at = now()`,
      [match.email],
    );

    const sessionToken = signSession({ email: match.email }, app.config.sessionSecret, SESSION_TTL_SECONDS);
    reply.setCookie(SESSION_COOKIE, sessionToken, {
      httpOnly: true, secure: !req.headers.host?.startsWith('localhost'),
      sameSite: 'lax', path: '/', maxAge: SESSION_TTL_SECONDS,
    });
    return reply.redirect(`${app.config.webOrigin}/#/wallet`, 302);
  });

  app.post('/auth/logout', async (req, reply) => {
    reply.clearCookie(SESSION_COOKIE, { path: '/' });
    return { ok: true };
  });
}

export function readSession(req: { cookies: Record<string, string | undefined> }, secret: string): { email: string } | null {
  const tok = req.cookies[SESSION_COOKIE];
  if (!tok) return null;
  return verifySession(tok, secret);
}
