// What a page hears over the socket: on connect, and after an edit, a new file and a deletion.
// usage: node wscompare.mjs <http-url> <repo-dir>      prints one JSON document
import { appendFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [url, repo] = process.argv.slice(2);
const SETTLE_MS = 1500;
const WAIT_MS = 6000;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const ws = new WebSocket(url.replace('http', 'ws') + '/ws');
let heard = [];
ws.onmessage = (e) => {
  try {
    const m = JSON.parse(String(e.data));
    heard.push({ type: m.type, keys: Object.keys(m).sort(), version: m.version ?? null });
  } catch {
    heard.push({ type: 'unparseable', keys: [], version: null });
  }
};
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = () => reject(new Error('socket failed'));
});

const phase = async (act) => {
  heard = [];
  await act?.();
  const start = Date.now();
  // Quiet for a settle period after the last message, or the wait runs out.
  let last = heard.length;
  let lastChange = Date.now();
  while (Date.now() - start < WAIT_MS && Date.now() - lastChange < SETTLE_MS) {
    await sleep(100);
    if (heard.length !== last) { last = heard.length; lastChange = Date.now(); }
  }
  return heard.map(({ type, keys }) => ({ type, keys }));
};

const tracked = readdirSync(repo).find((n) => /\.(md|json|ts|rs|go)$/.test(n)) ?? 'README.md';
const out = {};
out.initial = await phase();
out.afterEdit = await phase(() => appendFileSync(join(repo, tracked), '\n'));
out.afterCreate = await phase(() => writeFileSync(join(repo, 'ws-probe-new-file.txt'), 'hello\n'));
out.afterDelete = await phase(() => rmSync(join(repo, 'ws-probe-new-file.txt')));
out.afterBadFrame = await phase(() => ws.send('this is not json'));
out.stillOpen = ws.readyState === WebSocket.OPEN;
console.log(JSON.stringify(out));
ws.close();
process.exit(0);
