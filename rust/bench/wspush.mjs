// Edit a file, measure how long until the server pushes a new snapshot over the socket.
// usage: node wspush.mjs <http-url> <repo-dir> <changed-file> <single-edits> <bulk-files>
import { appendFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const [url, repo, file, singles, bulk] = process.argv.slice(2);
const ws = new WebSocket(url.replace('http', 'ws') + '/ws');
let waiter = null;
let snapshots = 0;
ws.onmessage = (e) => {
  let msg;
  try { msg = JSON.parse(e.data); } catch { return; }
  if (msg.type === 'snapshot') { snapshots += 1; waiter?.(performance.now()); }
};
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = () => reject(new Error('socket failed')); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const next = (timeout) => new Promise((resolve) => {
  const timer = setTimeout(() => { waiter = null; resolve(null); }, timeout);
  waiter = (t) => { clearTimeout(timer); waiter = null; resolve(t); };
});
await sleep(1500);

const single = [];
for (let i = 0; i < Number(singles); i++) {
  const t0 = performance.now();
  const pushed = next(20000);
  appendFileSync(join(repo, file), `// edit ${i}\n`);
  const t1 = await pushed;
  single.push(t1 === null ? null : Math.round(t1 - t0));
  await sleep(1200);
}
const ok = single.filter((x) => x !== null).sort((a, b) => a - b);

// Many files written at once: time to the first push and to the last one.
const files = [];
const walk = (dir) => { for (const d of readdirSync(dir, { withFileTypes: true })) {
  const p = join(dir, d.name);
  if (d.isDirectory()) { if (d.name !== '.git' && d.name !== 'node_modules') walk(p); } else if (/\.(rs|ts|json|md)$/.test(d.name)) files.push(p);
} };
walk(repo);
const bulkFiles = files.slice(0, Number(bulk));
const before = snapshots;
const t0 = performance.now();
const first = next(30000);
for (const f of bulkFiles) appendFileSync(f, '// bulk\n');
const tFirst = await first;
let tLast = tFirst;
for (;;) { const t = await next(3000); if (t === null) break; tLast = t; }
console.log(JSON.stringify({
  single_ms: { n: ok.length, missed: single.length - ok.length, median: ok[Math.floor(ok.length / 2)] ?? null, max: ok[ok.length - 1] ?? null, min: ok[0] ?? null },
  bulk: { files: bulkFiles.length, first_push_ms: tFirst === null ? null : Math.round(tFirst - t0), last_push_ms: tLast === null ? null : Math.round(tLast - t0), pushes: snapshots - before },
}));
ws.close();
process.exit(0);
