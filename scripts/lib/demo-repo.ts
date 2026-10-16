// The repository the README demo reviews: a URL shortener on `main`, and a `rate-limit` branch that
// adds a token bucket. It is the same repository diffle.app records its videos from
// (`scripts/demo-repo.ts` in glattziehen/diffle-website), so the README and the site tell one story.
// Author, dates, and content are fixed, so every run produces the same commits and the same diff.
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

export const BASE = 'main';
export const FEATURE = 'rate-limit';

/** File contents by path. */
type Files = Record<string, string>;

const MAIN_FILES: Files = {
  'README.md': `# shortlink

A tiny URL shortener.

\`\`\`bash
pixi run serve
\`\`\`
`,
  'pyproject.toml': `[project]
name = "shortlink"
version = "0.3.0"
requires-python = ">=3.12"
dependencies = ["starlette>=0.46", "uvicorn>=0.34"]
`,
  'shortlink/__init__.py': `from shortlink.app import app

__all__ = ["app"]
`,
  'shortlink/store.py': `import secrets
from dataclasses import dataclass, field

ALPHABET = "abcdefghijkmnpqrstuvwxyz23456789"


@dataclass
class Link:
    code: str
    target: str
    hits: int = 0


@dataclass
class Store:
    links: dict[str, Link] = field(default_factory=dict)

    def shorten(self, target: str) -> Link:
        code = "".join(secrets.choice(ALPHABET) for _ in range(6))
        link = Link(code=code, target=target)
        self.links[code] = link
        return link

    def resolve(self, code: str) -> Link | None:
        link = self.links.get(code)
        if link is not None:
            link.hits += 1
        return link
`,
  'shortlink/app.py': `from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import JSONResponse, Response
from starlette.responses import RedirectResponse
from starlette.routing import Route

from shortlink.store import Store

store = Store()


def not_found() -> JSONResponse:
    return JSONResponse({"error": "unknown link"}, 404)


async def shorten(request: Request) -> JSONResponse:
    body = await request.json()
    link = store.shorten(body["url"])
    return JSONResponse({"code": link.code}, 201)


async def follow(request: Request) -> Response:
    link = store.resolve(request.path_params["code"])
    if link is None:
        return not_found()
    return RedirectResponse(link.target, 302)


app = Starlette(
    routes=[
        Route("/", shorten, methods=["POST"]),
        Route("/{code}", follow),
    ]
)
`,
  'tests/test_store.py': `from shortlink.store import Store


def test_shorten_then_resolve():
    store = Store()
    link = store.shorten("https://example.com")
    assert store.resolve(link.code) is link
    assert link.hits == 1


def test_unknown_code():
    assert Store().resolve("nope") is None
`,
};

const FEATURE_COMMITS: { message: string; files: Files }[] = [
  {
    message: 'feat: add a token-bucket rate limiter',
    files: {
      'shortlink/ratelimit.py': `import time
from dataclasses import dataclass, field


@dataclass
class Bucket:
    tokens: float
    updated: float


@dataclass
class RateLimiter:
    """Allow \`rate\` requests per second per client, with bursts up to \`burst\`."""

    rate: float = 5.0
    burst: int = 10
    buckets: dict[str, Bucket] = field(default_factory=dict)

    def allow(self, client: str) -> bool:
        """Spend one token from \`client\`'s bucket.

        Returns False, and spends nothing, when the bucket is empty.
        """
        now = time.time()
        bucket = self.buckets.setdefault(client, Bucket(self.burst, now))
        elapsed = now - bucket.updated
        bucket.tokens = min(self.burst, bucket.tokens + elapsed * self.rate)
        bucket.updated = now
        if bucket.tokens < 1:
            return False
        bucket.tokens -= 1
        return True

    def retry_after(self, client: str) -> float:
        bucket = self.buckets[client]
        return max(0.0, (1 - bucket.tokens) / self.rate)
`,
    },
  },
  {
    message: 'feat: rate-limit link creation per client',
    files: {
      'shortlink/app.py': `from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import JSONResponse, Response
from starlette.responses import RedirectResponse
from starlette.routing import Route

from shortlink.ratelimit import RateLimiter
from shortlink.store import Store

store = Store()
limiter = RateLimiter(rate=2.0, burst=5)


def not_found() -> JSONResponse:
    return JSONResponse({"error": "unknown link"}, 404)


async def shorten(request: Request) -> JSONResponse:
    forwarded = request.headers.get("x-forwarded-for")
    client = forwarded or request.client.host
    if not limiter.allow(client):
        retry = limiter.retry_after(client)
        return JSONResponse(
            {"error": "slow down"},
            status_code=429,
            headers={"retry-after": str(int(retry))},
        )
    body = await request.json()
    link = store.shorten(body["url"])
    return JSONResponse({"code": link.code}, 201)


async def follow(request: Request) -> Response:
    link = store.resolve(request.path_params["code"])
    if link is None:
        return not_found()
    return RedirectResponse(link.target, 302)


async def stats(request: Request) -> JSONResponse:
    link = store.links.get(request.path_params["code"])
    if link is None:
        return not_found()
    return JSONResponse({"hits": link.hits})


app = Starlette(
    routes=[
        Route("/", shorten, methods=["POST"]),
        Route("/{code}/stats", stats),
        Route("/{code}", follow),
    ]
)
`,
      'shortlink/store.py': `import secrets
from dataclasses import dataclass, field

ALPHABET = "abcdefghijkmnpqrstuvwxyz23456789"
CODE_LENGTH = 7


@dataclass
class Link:
    code: str
    target: str
    hits: int = 0


@dataclass
class Store:
    links: dict[str, Link] = field(default_factory=dict)

    def shorten(self, target: str) -> Link:
        if not target.startswith(("http://", "https://")):
            raise ValueError(f"not a web address: {target}")
        code = "".join(secrets.choice(ALPHABET) for _ in range(CODE_LENGTH))
        link = Link(code=code, target=target)
        self.links[code] = link
        return link

    def resolve(self, code: str) -> Link | None:
        link = self.links.get(code)
        if link is not None:
            link.hits += 1
        return link
`,
    },
  },
  {
    message: 'test: cover the rate limiter',
    files: {
      'tests/test_ratelimit.py': `from shortlink.ratelimit import RateLimiter


def test_burst_then_block(monkeypatch):
    monkeypatch.setattr("time.time", lambda: 100.0)
    limiter = RateLimiter(rate=1.0, burst=3)
    assert [limiter.allow("a") for _ in range(4)] == [True, True, True, False]


def test_clients_are_independent(monkeypatch):
    monkeypatch.setattr("time.time", lambda: 100.0)
    limiter = RateLimiter(rate=1.0, burst=1)
    assert limiter.allow("a")
    assert limiter.allow("b")


def test_refills_over_time(monkeypatch):
    now = [100.0]
    monkeypatch.setattr("time.time", lambda: now[0])
    limiter = RateLimiter(rate=2.0, burst=1)
    assert limiter.allow("a")
    assert not limiter.allow("a")
    now[0] += 0.5
    assert limiter.allow("a")
`,
      'README.md': `# shortlink

A tiny URL shortener.

\`\`\`bash
pixi run serve
\`\`\`

## Rate limits

Creating links is limited to 2 per second per client, with bursts of 5.
Over the limit, the API answers \`429\` with a \`Retry-After\` header.
`,
    },
  },
];

function writeFiles(repo: string, files: Files) {
  for (const [path, content] of Object.entries(files)) {
    mkdirSync(dirname(join(repo, path)), { recursive: true });
    writeFileSync(join(repo, path), content);
  }
}

/** Create the demo repository in `repo` (a new or empty directory), checked out on the feature branch. */
export function buildDemoRepo(repo: string) {
  mkdirSync(repo, { recursive: true });
  let tick = 0;
  const git = (...args: string[]) => {
    // One minute per commit from a fixed epoch keeps hashes identical across runs.
    const date = new Date(Date.UTC(2026, 8, 14, 9, tick)).toISOString();
    execFileSync('git', args, {
      cwd: repo,
      stdio: 'ignore',
      env: {
        ...process.env,
        GIT_CONFIG_GLOBAL: '/dev/null',
        GIT_CONFIG_SYSTEM: '/dev/null',
        GIT_AUTHOR_NAME: 'Sam Reviewer',
        GIT_AUTHOR_EMAIL: 'sam@example.com',
        GIT_COMMITTER_NAME: 'Sam Reviewer',
        GIT_COMMITTER_EMAIL: 'sam@example.com',
        GIT_AUTHOR_DATE: date,
        GIT_COMMITTER_DATE: date,
      },
    });
  };
  const commit = (message: string) => {
    git('add', '-A');
    git('commit', '--quiet', '--no-gpg-sign', '-m', message);
    tick++;
  };

  git('init', '--quiet', '--initial-branch', BASE);
  writeFiles(repo, MAIN_FILES);
  commit('feat: shorten and follow links');
  git('checkout', '--quiet', '-b', FEATURE);
  for (const { message, files } of FEATURE_COMMITS) {
    writeFiles(repo, files);
    commit(message);
  }
}
