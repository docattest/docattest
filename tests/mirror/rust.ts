// The mirror test's scenarios in Rust. One crate, `fixture`; each TypeScript module is one
// of its modules. The fixture keeps its names: a Rust item may be called `retry`, and a
// constant `RETRY_DEFAULTS`. A test's title is its function's name.
import { lines } from '../lib/lines.ts';
import { attempts, found, list, type Language } from './translation.ts';

const LIB = 'src/lib.rs';
const DEFAULTS = (ts: string): string[] => [
  'pub struct RetryDefaults {',
  '    pub max_attempts: u32,',
  '}',
  '',
  `pub const RETRY_DEFAULTS: RetryDefaults = RetryDefaults { max_attempts: ${attempts(ts)} };`,
];

/** A TypeScript array of strings as a Rust constant. */
const strings = (name: string, ts: string): string =>
  `pub const ${name}: &[&str] = &${list(ts).replaceAll("'", '"')};`;

/** src/index.ts, which is a different module in each group of scenarios. */
function index(ts: string): [path: string, text: string] {
  if (ts.includes("from './retry.js'")) {
    return [
      LIB,
      lines(
        'mod backoff;',
        'mod jitter;',
        'mod parse;',
        'mod retry;',
        'mod unused;',
        '',
        'pub use parse::parse;',
        'pub use retry::{retry, RETRY_DEFAULTS};',
      ),
    ];
  }
  for (const name of ['ENV_KEYS', 'ROUTES']) {
    if (ts.includes(name)) return [LIB, lines(strings(name, ts))];
  }
  if (ts.includes('ErrorCodes')) {
    return [
      LIB,
      lines(
        '#[allow(non_upper_case_globals)]',
        'pub const ErrorCodes: &[(&str, &str)] =',
        '    &[("RPC_ERROR", "rpc"), ("NO_SESSION", "session"), ("TIMEOUT", "timeout"), ("AUTH_ERROR", "auth")];',
      ),
    ];
  }
  if (ts.includes('a-package-that-is-not-installed')) {
    return [LIB, lines('use a_package_that_is_not_installed as _;', '', strings('REGIONS', ts))];
  }
  if (ts.includes('REGIONS')) return [LIB, lines(strings('REGIONS', ts))];
  if (ts.includes('RETRY_DEFAULTS')) {
    return [
      LIB,
      lines(
        ...DEFAULTS(ts),
        '',
        'pub fn retry<T, E>(call: impl Fn() -> Result<T, E>) -> Result<T, E> {',
        '    call()',
        '}',
      ),
    ];
  }
  throw new Error(`no translation for src/index.ts: ${ts}`);
}

const OK = 'retry(|| Ok::<_, String>("ok"))';

export const rust: Language = {
  adapter: 'rust',
  label: 'rs',
  manifest: ['Cargo.toml', '[package]\nname = "fixture"\nversion = "1.0.0"\nedition = "2021"\n'],
  surface: 'fixture',
  page: [
    ['```ts ', '```rust '],
    ["import { retry } from 'fixture';", 'use fixture::retry;'],
    ["import { parse } from 'fixture';", 'use fixture::parse;'],
    ["assert.equal(await retry(async () => 'ok'), 'ok');", `assert_eq!(${OK}, Ok("ok"));`],
    [
      'assert.equal(await retry(async () => 7), 7);',
      'assert_eq!(retry(|| Ok::<_, String>(7)), Ok(7));',
    ],
    ["await retry(async () => 'ok');", `let _ = ${OK};`],
    ["assert.equal(parse('4'), 4);", 'assert_eq!(parse("4"), 4.0);'],
    ['Math.min(RETRY_DEFAULTS.maxAttempts, 3)', 'RETRY_DEFAULTS.max_attempts.min(3)'],
    ['RETRY_DEFAULTS.maxAttempts', 'RETRY_DEFAULTS.max_attempts'],
    ['test/retry.test.ts > "throws the last error"', 'tests/retry.rs > "throws_the_last_error"'],
  ],
  code: {
    'src/index.ts': index,
    'src/retry.ts': (ts) => [
      'src/retry.rs',
      lines(
        ...(ts.startsWith('// Retries a call.') ? ['// Retries a call.'] : []),
        'use crate::backoff::backoff;',
        'use std::time::Duration;',
        '',
        ...DEFAULTS(ts),
        '',
        'pub fn retry<T, E>(call: impl Fn() -> Result<T, E>) -> Result<T, E> {',
        '    let mut attempt = 1;',
        '    loop {',
        '        match call() {',
        '            Err(_) if attempt < RETRY_DEFAULTS.max_attempts => {',
        '                std::thread::sleep(Duration::from_millis(backoff(attempt)));',
        '            }',
        '            done => return done,',
        '        }',
        '        attempt += 1;',
        '    }',
        '}',
      ),
    ],
    'src/backoff.ts': (ts) => [
      'src/backoff.rs',
      lines(
        'use crate::jitter::jitter;',
        '',
        'pub fn backoff(attempt: u32) -> u64 {',
        `    jitter(u64::from(attempt) * ${found(/attempt \* (\d+)/, ts, 1)})`,
        '}',
      ),
    ],
    'src/jitter.ts': (ts) => [
      'src/jitter.rs',
      lines(
        'pub fn jitter(ms: u64) -> u64 {',
        ts.includes('Math.random') ? '    ms + u64::from(std::process::id() % 2)' : '    ms',
        '}',
      ),
    ],
    'src/parse.ts': (ts) => [
      'src/parse.rs',
      lines(
        'pub fn parse(text: &str) -> f64 {',
        ts.includes('parseInt')
          ? '    text.parse::<i64>().map_or(f64::NAN, |whole| whole as f64)'
          : '    text.parse().unwrap_or(f64::NAN)',
        '}',
      ),
    ],
    'src/unused.ts': () => ['src/unused.rs', lines('pub fn unused() -> u32 {', '    1', '}')],
    'test/retry.test.ts': (ts) => [
      'tests/retry.rs',
      lines(
        'use fixture::retry;',
        '',
        '#[test]',
        `fn ${ts.includes("it('rethrows'") ? 'rethrows' : 'throws_the_last_error'}() {`,
        '    assert_eq!(retry(|| Err::<(), _>("x")), Err("x"));',
        '}',
      ),
    ],
  },
};
