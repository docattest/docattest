// The mirror test's scenarios in Go. One package, `fixture`, at the root of a module of
// the same name; each TypeScript module is one of its files. A Go export begins with a
// capital, so `retry` and `parse` are `Retry` and `Parse` here, and what the core prints
// about them is restated in the scenario's names before its `expect` judges it. The module
// requires this checkout's, as a module that uses its `cells` package does.
import { join } from 'node:path';
import { ROOT } from '../../scripts/lib/repository.ts';
import { lines } from '../lib/lines.ts';
import { attempts, found, list, type Language } from './translation.ts';

const PACKAGE = 'package fixture';
const DEFAULTS = (ts: string): string =>
  `var RETRY_DEFAULTS = struct{ MaxAttempts int }{MaxAttempts: ${attempts(ts)}}`;

/** A TypeScript array of strings as a Go variable. */
const strings = (name: string, ts: string): string =>
  `var ${name} = []string{${list(ts).slice(1, -1).replaceAll("'", '"')}}`;

/** src/index.ts, which is a different module in each group of scenarios. */
function index(ts: string): [path: string, text: string] {
  const at = 'fixture.go';
  // What only re-exports has nothing to say in Go, where a package is all its files.
  if (ts.includes("from './retry.js'")) return [at, lines(PACKAGE)];
  for (const name of ['ENV_KEYS', 'ROUTES']) {
    if (ts.includes(name)) return [at, lines(PACKAGE, '', strings(name, ts))];
  }
  if (ts.includes('ErrorCodes')) {
    return [
      at,
      lines(
        PACKAGE,
        '',
        'var ErrorCodes = map[string]string{"RPC_ERROR": "rpc", "NO_SESSION": "session", "TIMEOUT": "timeout", "AUTH_ERROR": "auth"}',
      ),
    ];
  }
  if (ts.includes('a-package-that-is-not-installed')) {
    return [
      at,
      lines(PACKAGE, '', 'import _ "a-package-that-is-not-installed"', '', strings('REGIONS', ts)),
    ];
  }
  if (ts.includes('REGIONS')) return [at, lines(PACKAGE, '', strings('REGIONS', ts))];
  if (ts.includes('RETRY_DEFAULTS')) {
    return [
      at,
      lines(
        PACKAGE,
        '',
        DEFAULTS(ts),
        '',
        'func Retry[T any](fn func() (T, error)) (T, error) {',
        '\treturn fn()',
        '}',
      ),
    ];
  }
  throw new Error(`no translation for src/index.ts: ${ts}`);
}

const OK = 'func() (string, error) { return "ok", nil }';

const ADAPTER_MODULE = 'github.com/docattest/docattest/adapters/go';
const GO_MOD = lines(
  'module fixture',
  '',
  'go 1.23',
  '',
  `require ${ADAPTER_MODULE} v0.0.0`,
  '',
  `replace ${ADAPTER_MODULE} => ${join(ROOT, 'adapters/go')}`,
);

/** The translations of the mirror's own scenarios (scenarios.ts). */
const AGREEMENTS: Language['code'] = {
  'src/server.ts': () => [
    'fixture.go',
    lines(
      PACKAGE,
      '',
      'const port = 8080',
      '',
      'const NAME = "shop"',
      '',
      'func secret() int { return 7 }',
    ),
  ],
  'src/main.ts': () => [
    'cmd/shopd/main.go',
    lines(
      'package main',
      '',
      'const port = 8080',
      '',
      'func main() { panic("shopd needs a terminal") }',
    ),
  ],
  'src/settings.ts': () => [
    'fixture.go',
    lines(PACKAGE, '', 'const STRICT = true', '', 'const RATIO = 1.0', '', 'var NOTHING any'),
  ],
  'src/rates.ts': () => [
    'fixture.go',
    lines(PACKAGE, '', 'const RATE = 3', '', 'const QUOTE = 2', '', 'const DEBUG_LEVEL = 1'),
  ],
  'test/price.test.ts': () => [
    'price_test.go',
    lines(
      PACKAGE,
      '',
      'import "testing"',
      '',
      'func TestPrice(t *testing.T) {',
      '\tcases := []struct {',
      '\t\tname  string',
      '\t\tcents int',
      '\t}{{"rounds down", 1}, {"rounds up", 5}}',
      '\tfor _, c := range cases {',
      '\t\tt.Run(c.name, func(t *testing.T) {})',
      '\t}',
      '}',
    ),
  ],
  'docs/docattest.setup.ts': () => [
    'docs/docattest_setup.go',
    lines(
      'package docs',
      '',
      'import (',
      '\t"fmt"',
      '\t"strings"',
      '',
      `\t"${ADAPTER_MODULE}/cells"`,
      ')',
      '',
      'var Cases = map[string]func(row map[string]string) error{"spans": spans}',
      '',
      'func spans(row map[string]string) error {',
      '\tif got := strings.Join(cells.CodeSpans(row["cell"]), ","); got != row["spans"] {',
      '\t\treturn fmt.Errorf("expected %s, got %s", row["spans"], got)',
      '\t}',
      '\treturn nil',
      '}',
    ),
  ],
};

export const go: Language = {
  adapter: 'go',
  label: 'go',
  manifest: ['go.mod', GO_MOD],
  surface: '.',
  programs: { './src/main.ts': './cmd/shopd' },
  page: [
    ['```ts ', '```go '],
    ["import { retry } from 'fixture';", 'import "fixture"'],
    ["import { parse } from 'fixture';", 'import "fixture"'],
    [
      "assert.equal(await retry(async () => 'ok'), 'ok');",
      `if v, _ := fixture.Retry(${OK}); v != "ok" { panic(v) }`,
    ],
    [
      'assert.equal(await retry(async () => 7), 7);',
      'if v, _ := fixture.Retry(func() (int, error) { return 7, nil }); v != 7 { panic(v) }',
    ],
    ["await retry(async () => 'ok');", `_, _ = fixture.Retry(${OK})`],
    ["assert.equal(parse('4'), 4);", 'if fixture.Parse("4") != 4 { panic("4") }'],
    ['Math.min(RETRY_DEFAULTS.maxAttempts, 3)', 'min(RETRY_DEFAULTS.MaxAttempts, 3)'],
    ['RETRY_DEFAULTS.maxAttempts', 'RETRY_DEFAULTS.MaxAttempts'],
    ['with `parse` before', 'with `Parse` before'],
    // A Go subtest keeps the title; its file is beside the code it tests.
    ['test/retry.test.ts > ', 'retry_test.go > '],
    ['test/price.test.ts > "price" > ', 'price_test.go > "TestPrice" > '],
    // An unexported Go name begins with a small letter.
    ['fact: PORT', 'fact: port'],
    ['@cli: PORT', '@cli: port'],
  ],
  code: {
    ...AGREEMENTS,
    'src/index.ts': index,
    'src/retry.ts': (ts) => [
      'retry.go',
      lines(
        ...(ts.startsWith('// Retries a call.') ? ['// Retries a call.'] : []),
        PACKAGE,
        '',
        'import "time"',
        '',
        DEFAULTS(ts),
        '',
        'func Retry[T any](fn func() (T, error)) (T, error) {',
        '\tfor attempt := 1; ; attempt++ {',
        '\t\tvalue, err := fn()',
        '\t\tif err == nil || attempt >= RETRY_DEFAULTS.MaxAttempts {',
        '\t\t\treturn value, err',
        '\t\t}',
        '\t\ttime.Sleep(time.Duration(backoff(attempt)) * time.Millisecond)',
        '\t}',
        '}',
      ),
    ],
    'src/backoff.ts': (ts) => [
      'backoff.go',
      lines(
        PACKAGE,
        '',
        'func backoff(attempt int) int {',
        `\treturn jitter(attempt * ${found(/attempt \* (\d+)/, ts, 1)})`,
        '}',
      ),
    ],
    'src/jitter.ts': (ts) => [
      'jitter.go',
      ts.includes('Math.random')
        ? lines(
            PACKAGE,
            '',
            'import "math/rand"',
            '',
            'func jitter(ms int) int {',
            '\treturn ms + rand.Intn(2)',
            '}',
          )
        : lines(PACKAGE, '', 'func jitter(ms int) int {', '\treturn ms', '}'),
    ],
    'src/parse.ts': (ts) => [
      'parse.go',
      lines(
        PACKAGE,
        '',
        'import "strconv"',
        '',
        'func Parse(text string) float64 {',
        ts.includes('parseInt')
          ? '\tn, _ := strconv.ParseInt(text, 10, 64)'
          : '\tn, _ := strconv.ParseFloat(text, 64)',
        '\treturn float64(n)',
        '}',
      ),
    ],
    'src/unused.ts': () => [
      'unused.go',
      lines(PACKAGE, '', 'func unused() int {', '\treturn 1', '}'),
    ],
    'test/retry.test.ts': (ts) => [
      'retry_test.go',
      lines(
        PACKAGE,
        '',
        'import (',
        '\t"errors"',
        '\t"testing"',
        ')',
        '',
        'func TestRetry(t *testing.T) {',
        `\tt.Run("${ts.includes("it('rethrows'") ? 'rethrows' : 'throws the last error'}", func(t *testing.T) {`,
        '\t\t_, err := Retry(func() (int, error) { return 0, errors.New("x") })',
        '\t\tif err == nil || err.Error() != "x" {',
        '\t\t\tt.Fatal(err)',
        '\t\t}',
        '\t})',
        '}',
      ),
    ],
  },
  restate: [
    ['"Retry"', '"retry"'],
    ['"Parse"', '"parse"'],
  ],
};
