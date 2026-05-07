import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Panel } from '../components/Panel.js';
import { useMe } from '../hooks/useMe.js';
import { api } from '../api.js';

type Status = 'idle' | 'mining' | 'submitting' | 'minted' | 'error';

export function MinePage() {
  const { me, loading, refresh } = useMe();
  const nav = useNavigate();
  const [status, setStatus] = useState<Status>('idle');
  const [target, setTarget] = useState<number | null>(null);
  const [hashes, setHashes] = useState('0');
  const [elapsed, setElapsed] = useState(0);
  const [error, setError] = useState('');
  const [tokenId, setTokenId] = useState('');
  const workerRef = useRef<Worker | null>(null);

  useEffect(() => () => workerRef.current?.terminate(), []);

  async function start() {
    if (!me) { nav('/login'); return; }
    setStatus('mining'); setError(''); setHashes('0'); setElapsed(0);
    const ch = await api.challenge();
    setTarget(ch.difficulty_bits);
    const w = new Worker(new URL('../miner.worker.ts', import.meta.url), { type: 'module' });
    workerRef.current = w;
    w.onmessage = async (e: MessageEvent<any>) => {
      const m = e.data;
      if (m.type === 'progress') { setHashes(m.hashes); setElapsed(m.elapsed_ms); return; }
      if (m.type === 'aborted') { setStatus('idle'); return; }
      if (m.type === 'found') {
        setStatus('submitting');
        try {
          const r = await api.mint({ challenge_id: ch.challenge_id, solution_nonce: m.solution_nonce });
          setTokenId(r.token.id);
          setStatus('minted');
          await refresh();
        } catch (err: any) {
          setStatus('error');
          setError(err?.message ?? 'mint failed');
        } finally { w.terminate(); workerRef.current = null; }
      }
    };
    w.postMessage({ type: 'start', nonce_prefix: ch.nonce_prefix, difficulty_bits: ch.difficulty_bits });
  }

  function abort() {
    workerRef.current?.postMessage({ type: 'abort' });
  }

  function fmtRate() {
    if (!elapsed) return '0';
    const h = Number(hashes);
    const mhs = (h / 1e6) / (elapsed / 1000);
    return mhs.toFixed(2) + ' MH/s';
  }
  function fmtElapsed() {
    const s = Math.floor(elapsed / 1000);
    const mm = String(Math.floor(s / 60)).padStart(2, '0');
    const ss = String(s % 60).padStart(2, '0');
    return `00:${mm}:${ss}`;
  }

  if (loading) return <Panel><div>loading...</div></Panel>;
  if (!me) return <Panel title="MINE"><div>not signed in.</div></Panel>;

  return (
    <Panel title="MINE">
      <pre style={{ margin: 0 }}>
{`  TARGET    : ${target ?? '--'} trailing zero bits
  HASHES    : ${Number(hashes).toLocaleString()}
  RATE      : ${fmtRate()}
  ELAPSED   : ${fmtElapsed()}
  STATUS    : ${status.toUpperCase()}${tokenId ? `\n  TOKEN     : ${tokenId}` : ''}${error ? `\n  ERROR     : ${error}` : ''}
`}
      </pre>
      <div style={{ marginTop: 8 }}>
        {status === 'idle' || status === 'minted' || status === 'error' ? (
          <button onClick={start}>[ MINE ]</button>
        ) : (
          <button onClick={abort}>[ ABORT ]</button>
        )}
      </div>
    </Panel>
  );
}
