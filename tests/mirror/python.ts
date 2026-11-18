// The mirror test's scenarios in Python. The fixture keeps its names: a Python module may
// export `retry` and `RETRY_DEFAULTS` as they are.
import { lines } from '../lib/lines.ts';
import { attempts, found, list, type Language } from './translation.ts';

const INIT = 'src/fixture/__init__.py';

/** src/index.ts, which is a different module in each group of scenarios. */
function index(ts: string): [path: string, text: string] {
  if (ts.includes("from './retry.js'")) {
    return [INIT, lines('from .parse import parse', 'from .retry import RETRY_DEFAULTS, retry')];
  }
  if (ts.includes('ENV_KEYS')) return [INIT, `ENV_KEYS = ${list(ts)}\n`];
  if (ts.includes('ROUTES')) return [INIT, `ROUTES = ${list(ts)}\n`];
  if (ts.includes('ErrorCodes')) {
    return [
      INIT,
      'ErrorCodes = {"RPC_ERROR": "rpc", "NO_SESSION": "session", "TIMEOUT": "timeout", "AUTH_ERROR": "auth"}\n',
    ];
  }
  if (ts.includes('a-package-that-is-not-installed')) {
    return [INIT, `import a_package_that_is_not_installed\n\nREGIONS = ${list(ts)}\n`];
  }
  if (ts.includes('REGIONS')) return [INIT, `REGIONS = ${list(ts)}\n`];
  if (ts.includes('RETRY_DEFAULTS')) {
    return [
      INIT,
      lines(
        `RETRY_DEFAULTS = {"max_attempts": ${attempts(ts)}}`,
        '',
        '',
        'async def retry(fn):',
        '    return await fn()',
      ),
    ];
  }
  throw new Error(`no translation for src/index.ts: ${ts}`);
}

/** The translations of the mirror's own scenarios (scenarios.ts). */
const AGREEMENTS: Language['code'] = {
  // A name left out of `__all__` is not exported, so the names stay as they are.
  'src/server.ts': () => [
    INIT,
    lines(
      '__all__ = ["NAME"]',
      '',
      'PORT = 8080',
      'NAME = "shop"',
      '',
      '',
      'def secret():',
      '    return 7',
    ),
  ],
  'src/main.ts': () => [
    'src/shopd/__main__.py',
    lines('PORT = 8080', 'raise SystemExit("shopd needs a terminal")'),
  ],
  'src/settings.ts': () => [INIT, lines('STRICT = True', 'RATIO = 1.0', 'NOTHING = None')],
  'src/rates.ts': () => [INIT, lines('RATE = 3', 'QUOTE = 2', 'DEBUG_LEVEL = 1')],
  'test/price.test.ts': () => [
    'tests/test_price.py',
    lines(
      'import pytest',
      '',
      '',
      '@pytest.mark.parametrize("cents", [1, 5], ids=["rounds down", "rounds up"])',
      'def test_price(cents):',
      '    assert cents > 0',
    ),
  ],
  'docs/docattest.setup.ts': () => [
    'docs/docattest_setup.py',
    lines(
      'from docattest.cells import code_spans',
      '',
      '',
      'def spans(row):',
      '    assert ",".join(code_spans(row["cell"])) == row["spans"]',
      '',
      '',
      'cases = {"spans": spans}',
    ),
  ],
};

export const python: Language = {
  adapter: 'python',
  label: 'py',
  manifest: ['pyproject.toml', '[project]\nname = "fixture"\nversion = "1.0.0"\n'],
  surface: './src/fixture',
  programs: { './src/main.ts': './src/shopd/__main__.py' },
  page: [
    ['```ts ', '```python '],
    ["import { retry } from 'fixture';", 'import asyncio; from fixture import retry'],
    [
      "assert.equal(await retry(async () => 'ok'), 'ok');",
      'assert await retry(lambda: asyncio.sleep(0, "ok")) == "ok"',
    ],
    [
      'assert.equal(await retry(async () => 7), 7);',
      'assert await retry(lambda: asyncio.sleep(0, 7)) == 7',
    ],
    ["await retry(async () => 'ok');", 'await retry(lambda: asyncio.sleep(0, "ok"))'],
    ["import { parse } from 'fixture';", 'from fixture import parse'],
    ["assert.equal(parse('4'), 4);", "assert parse('4') == 4"],
    ['Math.min(RETRY_DEFAULTS.maxAttempts, 3)', 'min(RETRY_DEFAULTS["max_attempts"], 3)'],
    ['RETRY_DEFAULTS.maxAttempts', 'RETRY_DEFAULTS["max_attempts"]'],
    // A Python test's title is its function's name.
    [
      'test/retry.test.ts > "throws the last error"',
      'tests/test_retry.py > "test_throws_the_last_error"',
    ],
    ['test/price.test.ts > "price" > ', 'tests/test_price.py > "test_price" > '],
  ],
  code: {
    ...AGREEMENTS,
    'src/index.ts': index,
    'src/retry.ts': (ts) => [
      'src/fixture/retry.py',
      lines(
        ...(ts.startsWith('// Retries a call.') ? ['# Retries a call.'] : []),
        'import asyncio',
        '',
        'from .backoff import backoff',
        '',
        `RETRY_DEFAULTS = {"max_attempts": ${attempts(ts)}}`,
        '',
        '',
        'async def retry(fn):',
        '    attempt = 1',
        '    while True:',
        '        try:',
        '            return await fn()',
        '        except Exception:',
        '            if attempt >= RETRY_DEFAULTS["max_attempts"]:',
        '                raise',
        '            await asyncio.sleep(backoff(attempt) / 1000)',
        '        attempt += 1',
      ),
    ],
    'src/backoff.ts': (ts) => [
      'src/fixture/backoff.py',
      lines(
        'from .jitter import jitter',
        '',
        '',
        'def backoff(attempt):',
        `    return jitter(attempt * ${found(/attempt \* (\d+)/, ts, 1)})`,
      ),
    ],
    'src/jitter.ts': (ts) => [
      'src/fixture/jitter.py',
      ts.includes('Math.random')
        ? lines('import random', '', '', 'def jitter(ms):', '    return ms + random.random()')
        : lines('def jitter(ms):', '    return ms'),
    ],
    'src/parse.ts': (ts) => [
      'src/fixture/parse.py',
      lines(
        'def parse(text):',
        ts.includes('parseInt') ? '    return int(text, 10)' : '    return float(text)',
      ),
    ],
    'src/unused.ts': () => ['src/fixture/unused.py', lines('def unused():', '    return 1')],
    'test/retry.test.ts': (ts) => [
      'tests/test_retry.py',
      lines(
        'import asyncio',
        '',
        'import pytest',
        '',
        'from fixture import retry',
        '',
        '',
        'class TestRetry:',
        `    def ${ts.includes("it('rethrows'") ? 'test_rethrows' : 'test_throws_the_last_error'}(self):`,
        '        async def fails():',
        '            raise RuntimeError("x")',
        '',
        '        with pytest.raises(RuntimeError, match="x"):',
        '            asyncio.run(retry(fails))',
      ),
    ],
  },
};
