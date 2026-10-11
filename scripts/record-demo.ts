// Record the README's demo: `npm run demo`. A real diffle reviews the demo repository from
// `lib/demo-repo.ts`, driven by Playwright, once per theme. Writes `.github/assets/demo-<theme>.mp4`
// and a smaller `.gif` of the same take, which is what the README embeds: GitHub renders neither
// `<video>` nor repo-relative mp4 in a README.
//
// Needs ffmpeg on PATH (with libx264) and Playwright's Chromium (`npm run test:e2e:install`).
// `KEEP_FRAMES=1` keeps the raw frames and prints where.
import type { Page } from '@playwright/test';
import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { openBrowser, settle } from '../test/e2e/browser.js';
import { REPO_ROOT, buildClient, quiet, withDiffle } from '../test/e2e/server.js';
import { rmTmp } from '../test/tmp.js';
import { BASE, FEATURE, buildDemoRepo } from './lib/demo-repo.js';

const OUT = join(REPO_ROOT, '.github/assets');
const VIEWPORT = { width: 1280, height: 800 };
const SCALE = 2;
// Capturing a 2x frame takes longer than a video frame lasts, so the scene plays SLOW times slower
// than written (pauses, typing, CSS animations, the page's timers) and the encode speeds it back up.
const SLOW = Number(process.env.SLOW ?? 3);
const MP4 = { fps: 30, width: 1920 };
// One GIF pixel per CSS pixel: legible at GitHub's column width, and still under a megabyte.
const GIF = { fps: 15, width: 1280 };

// ---------------------------------------------------------------------------------------------
// Overlays: a pointer and the keys being pressed. Playwright's input is synthetic, so neither would
// appear in the recording otherwise. Both run in the page, so they are self-contained.

function overlays(slow: number) {
  const KEY_NAMES: Record<string, string | null> = {
    Enter: '↵',
    Escape: 'esc',
    ' ': 'space',
    Control: null,
    Shift: null,
    Meta: null,
    Alt: null,
  };
  const install = () => {
    const style = document.createElement('style');
    style.textContent = `
      #demo-pointer { position: fixed; left: 0; top: 0; width: 22px; height: 22px; z-index: 2147483647;
        pointer-events: none; transform: translate(-100px, -100px);
        filter: drop-shadow(0 1px 2px rgb(0 0 0 / .45)); }
      #demo-pointer.down svg { transform: scale(.85); transform-origin: 3px 3px; }
      #demo-keys { position: fixed; left: 20px; bottom: 20px; z-index: 2147483647;
        pointer-events: none; display: flex; gap: 12px; }
      #demo-keys kbd { font: 800 44px/1 ui-monospace, monospace; display: inline-flex; align-items: center;
        justify-content: center; min-width: 60px; height: 60px; padding: 0 16px; border-radius: 14px;
        color: #fff; background: rgb(17 24 39 / .88); border: 1px solid rgb(255 255 255 / .18);
        box-shadow: 0 12px 36px rgb(0 0 0 / .35); transition: opacity .35s ease, transform .35s ease; }
      #demo-keys kbd.gone { opacity: 0; transform: translateY(6px); }
    `;
    document.documentElement.append(style);
    const pointer = document.createElement('div');
    pointer.id = 'demo-pointer';
    pointer.innerHTML = `<svg width="22" height="22" viewBox="0 0 24 24"><path d="M3 2.5v17.2l4.7-4.4 3 6.9 3.1-1.3-3-6.8h6.5z" fill="#fff" stroke="#111" stroke-width="1.4" stroke-linejoin="round"/></svg>`;
    const keys = document.createElement('div');
    keys.id = 'demo-keys';
    document.documentElement.append(pointer, keys);

    addEventListener(
      'mousemove',
      (e) => (pointer.style.transform = `translate(${e.clientX - 3}px, ${e.clientY - 2}px)`),
      true,
    );
    addEventListener('mousedown', () => pointer.classList.add('down'), true);
    addEventListener('mouseup', () => pointer.classList.remove('down'), true);

    let last: (HTMLElement & { timer?: ReturnType<typeof setTimeout> }) | null = null;
    let lastAt = 0;
    addEventListener(
      'keydown',
      (e) => {
        const target = e.composedPath()[0];
        const typing =
          target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA)$/.test(target.tagName));
        const combo = e.ctrlKey || e.metaKey;
        if (typing && !combo && e.key !== 'Escape') return;
        const name = KEY_NAMES[e.key] === undefined ? e.key : KEY_NAMES[e.key];
        if (name === null) return;
        const label = `${e.ctrlKey ? '⌃' : ''}${e.metaKey ? '⌘' : ''}${name}`;
        // Keys pressed in quick succession read as one sequence, like `yy`.
        const now = performance.now();
        if (last && now - lastAt < 700 * slow && !last.classList.contains('gone')) last.textContent += ` ${label}`;
        else {
          last = document.createElement('kbd');
          last.textContent = label;
          keys.append(last);
          while (keys.children.length > 3) keys.firstElementChild?.remove();
        }
        lastAt = now;
        const shown = last;
        clearTimeout(shown.timer);
        shown.timer = setTimeout(() => {
          shown.classList.add('gone');
          setTimeout(() => shown.remove(), 400);
        }, 1100);
      },
      true,
    );
  };
  if (document.documentElement) install();
  else addEventListener('DOMContentLoaded', install);
}

/**
 * Stretch the page's timers by `slow`, as the animation playback rate stretches CSS animations, so
 * feedback a timer takes down (the check mark `yy` puts on the copy button) stays up as long in the
 * video as in diffle.
 */
function slowTimers(slow: number) {
  const setTimeout = window.setTimeout;
  window.setTimeout = ((handler: TimerHandler, delay = 0, ...args: unknown[]) =>
    setTimeout(handler, delay * slow, ...args)) as typeof window.setTimeout;
}

// ---------------------------------------------------------------------------------------------
// Page helpers, paced for a viewer rather than a test.

const pause = (page: Page, ms: number) => page.waitForTimeout(ms * SLOW);

async function glide(page: Page, x: number, y: number, steps = 28) {
  await page.mouse.move(x, y, { steps: steps * SLOW });
}

/** The file under the cursor; none before the first `J`, unlike the e2e helper's fallback. */
async function activePath(page: Page) {
  const row = page.locator('file-tree-container [data-item-type="file"][aria-selected="true"]').first();
  return (await row.count()) ? row.getAttribute('data-item-path') : null;
}

/** Walk the cursor to `path` with J, the way a reader moves through the review. */
async function walkTo(page: Page, path: string) {
  for (let i = 0; i < 12 && (await activePath(page)) !== path; i++) {
    await page.keyboard.press('J');
    await pause(page, 450);
  }
  if ((await activePath(page)) !== path) throw new Error(`could not walk to ${path}`);
  await pause(page, 400);
}

/** The on-screen number cell of new-side `line` in `path`. */
async function lineCell(page: Page, path: string, line: number) {
  const item = page
    .locator('diffs-container')
    .filter({ has: page.locator(`[slot="header-custom"] [data-path=${JSON.stringify(path)}]`) })
    .first();
  await item.locator('[data-column-number]').first().waitFor({ timeout: 15000 });
  const boxes = await item.locator('[data-column-number]').evaluateAll(
    (cells, wanted) =>
      cells
        .filter((c) => c.getAttribute('data-column-number') === wanted)
        .map((c) => c.getBoundingClientRect())
        .filter((b) => b.width > 0 && b.height > 0)
        .map(({ x, y, width, height }) => ({ x, y, width, height })),
    String(line),
  );
  // The rightmost cell is the new side's.
  const box = boxes.sort((a, b) => b.x - a.x)[0];
  if (!box) throw new Error(`line ${line} of ${path} is not rendered`);
  return box;
}

/** Drag the number column from `from` to `to`, which opens the composer on that range. */
async function selectLines(page: Page, path: string, from: number, to: number) {
  const start = await lineCell(page, path, from);
  await glide(page, start.x + start.width / 2, start.y + start.height / 2);
  await pause(page, 250);
  await page.mouse.down();
  const end = await lineCell(page, path, to);
  await glide(page, end.x + end.width / 2, end.y + end.height - 4, 14);
  await page.mouse.up();
  await pause(page, 350);
}

async function comment(page: Page, text: string) {
  const composer = page.getByPlaceholder('Leave a comment…');
  await composer.waitFor();
  await pause(page, 300);
  await composer.pressSequentially(text, { delay: 28 * SLOW });
  await pause(page, 500);
  await composer.press('Control+Enter');
  await pause(page, 900);
}

/** The 1-based line of `path` in the demo repository holding `text`. */
function lineOf(repo: string, path: string, text: string) {
  const index = readFileSync(join(repo, path), 'utf8')
    .split('\n')
    .findIndex((line) => line.includes(text));
  if (index === -1) throw new Error(`${path} has no line with ${text}`);
  return index + 1;
}

/** The core loop, as on diffle.app: read, comment on lines, copy every comment as one prompt. */
async function scene(page: Page, repo: string) {
  await pause(page, 1200);
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press('j');
    await pause(page, 220);
  }
  await walkTo(page, 'shortlink/app.py');
  const forwarded = lineOf(repo, 'shortlink/app.py', 'x-forwarded-for');
  await selectLines(page, 'shortlink/app.py', forwarded, forwarded + 1);
  await comment(
    page,
    '`x-forwarded-for` is client-controlled, so a new value per request dodges the limit. Only trust it behind our proxy.',
  );

  await walkTo(page, 'shortlink/ratelimit.py');
  const now = lineOf(repo, 'shortlink/ratelimit.py', 'time.time()');
  await selectLines(page, 'shortlink/ratelimit.py', now, now);
  await comment(page, 'Use `time.monotonic()`: a wall-clock jump would refill or drain every bucket.');

  await glide(page, VIEWPORT.width * 0.55, VIEWPORT.height * 0.3);
  await pause(page, 500);
  await page.keyboard.press('y');
  await pause(page, 90);
  await page.keyboard.press('y');
  // End while the copy-all button still shows its check mark, so the last frame shows the copy landed.
  await pause(page, 1600);
}

// ---------------------------------------------------------------------------------------------
// Recording

/** A captured screenshot, and when it was taken in the scene's own (unslowed) seconds. */
interface Frame {
  file: string;
  at: number;
}

/**
 * Capture screenshots back to back until stopped. Chromium's screencast caps frames at CSS pixels;
 * Playwright's screenshot honours the device scale factor, so text stays sharp.
 */
function capture(page: Page, dir: string): () => Promise<Frame[]> {
  const frames: Frame[] = [];
  let running = true;
  const loop = (async () => {
    while (running) {
      const file = join(dir, `${String(frames.length).padStart(5, '0')}.jpg`);
      const sent = performance.now();
      await page.screenshot({ path: file, type: 'jpeg', quality: 92, caret: 'initial' });
      frames.push({ file, at: (sent + performance.now()) / 2000 / SLOW });
    }
  })();
  return async () => {
    running = false;
    await loop;
    return frames;
  };
}

/** Encode frames, each shown until the next one arrived and the last held, as `<name>.mp4` and `<name>.gif`. */
function encode(frames: Frame[], dir: string, name: string, hold = 2.5) {
  if (frames.length < 2) throw new Error(`${name}: only ${frames.length} frames captured`);
  const lines = frames.flatMap((frame, i) => {
    const next = frames[i + 1];
    return [`file '${frame.file}'`, `duration ${Math.max(next ? next.at - frame.at : hold, 1 / MP4.fps).toFixed(4)}`];
  });
  // The concat demuxer ignores the last entry's duration unless the file is repeated.
  lines.push(`file '${frames.at(-1)!.file}'`);
  const list = join(dir, 'frames.txt');
  writeFileSync(list, lines.join('\n'));
  const input = ['-y', '-v', 'error', '-f', 'concat', '-safe', '0', '-i', list];

  const mp4 = join(OUT, `${name}.mp4`);
  quiet('ffmpeg', [
    ...input,
    '-vf',
    `fps=${MP4.fps},scale=${MP4.width}:-2:flags=lanczos`,
    '-c:v',
    'libx264',
    '-preset',
    'slow',
    '-crf',
    '24',
    '-pix_fmt',
    'yuv420p',
    '-movflags',
    '+faststart',
    '-an',
    mp4,
  ]);
  // One palette for the whole take, and only changed pixels per frame: the UI is mostly still.
  const gif = join(OUT, `${name}.gif`);
  quiet('ffmpeg', [
    ...input,
    '-vf',
    `fps=${GIF.fps},scale=${GIF.width}:-2:flags=lanczos,split[a][b];` +
      '[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle',
    gif,
  ]);
  for (const file of [mp4, gif])
    console.log(`${file.slice(REPO_ROOT.length + 1)} ${(statSync(file).size / 1e6).toFixed(1)} MB`);
}

async function record(browser: Awaited<ReturnType<typeof openBrowser>>, theme: 'dark' | 'light') {
  const work = await mkdtemp(join(tmpdir(), `diffle-demo-${theme}-`));
  const repo = join(work, 'shortlink');
  buildDemoRepo(repo);
  try {
    await withDiffle({ repo, revs: [`${BASE}...${FEATURE}`] }, async ({ url }) => {
      const context = await browser.newContext({
        viewport: VIEWPORT,
        deviceScaleFactor: SCALE,
        colorScheme: theme,
        locale: 'en-US',
        timezoneId: 'UTC',
        permissions: ['clipboard-read', 'clipboard-write'],
      });
      try {
        // tsx wraps named functions in `__name(...)`, which the page lacks when they are serialized into it.
        await context.addInitScript({ content: 'globalThis.__name = (fn) => fn;' });
        await context.addInitScript(slowTimers, SLOW);
        await context.addInitScript(overlays, SLOW);
        // Unified leaves the comment composer room at this width.
        await context.addInitScript(() => localStorage.setItem('diffle:diffStyle', 'unified'));
        const page = await context.newPage();
        await page.goto(url);
        await settle(page);
        await page.mouse.move(VIEWPORT.width * 0.6, VIEWPORT.height * 0.55);
        const cdp = await context.newCDPSession(page);
        await cdp.send('Animation.enable');
        await cdp.send('Animation.setPlaybackRate', { playbackRate: 1 / SLOW });

        const dir = join(work, 'frames');
        mkdirSync(dir);
        const stop = capture(page, dir);
        let frames: Frame[];
        try {
          await scene(page, repo);
        } finally {
          frames = await stop();
        }
        encode(frames, dir, `demo-${theme}`);
      } finally {
        await context.close();
      }
    });
  } finally {
    if (process.env.KEEP_FRAMES) console.log(`frames kept in ${work}`);
    else await rmTmp(work);
  }
}

buildClient();
mkdirSync(OUT, { recursive: true });
const browser = await openBrowser();
try {
  for (const theme of ['dark', 'light'] as const) await record(browser, theme);
} finally {
  await browser.close();
}
