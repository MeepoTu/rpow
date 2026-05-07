import { createSHA256 } from 'hash-wasm';
import { trailingZeroBits, bytesFromHex } from '@rpow/shared';

type StartMsg = { type: 'start'; nonce_prefix: string; difficulty_bits: number };
type AbortMsg = { type: 'abort' };
type InMsg = StartMsg | AbortMsg;

let aborted = false;

self.onmessage = async (e: MessageEvent<InMsg>) => {
  const msg = e.data;
  if (msg.type === 'abort') { aborted = true; return; }

  aborted = false;
  const prefix = bytesFromHex(msg.nonce_prefix);
  const target = msg.difficulty_bits;

  const sha = await createSHA256();
  const buf = new Uint8Array(prefix.length + 8);
  buf.set(prefix, 0);

  const startedAt = performance.now();
  let last = startedAt;
  let count = 0n;
  let nonce = 0n;

  while (!aborted) {
    let x = nonce;
    for (let i = 0; i < 8; i++) { buf[prefix.length + i] = Number(x & 0xffn); x >>= 8n; }
    sha.init();
    sha.update(buf);
    const digest = sha.digest('binary'); // Uint8Array
    if (trailingZeroBits(digest) >= target) {
      (self as any).postMessage({ type: 'found', solution_nonce: nonce.toString(), hashes: count.toString() });
      return;
    }
    nonce++;
    count++;
    if ((count & 0xffffn) === 0n) {
      const now = performance.now();
      if (now - last > 250) {
        (self as any).postMessage({ type: 'progress', hashes: count.toString(), elapsed_ms: Math.round(now - startedAt) });
        last = now;
      }
    }
  }
  (self as any).postMessage({ type: 'aborted' });
};
