// What `audit` matches: every target kind, unit-level cases of the matching rules, and its
// suggestions through real surfaces.
import { lines } from '../../lib/lines.ts';
import {
  config,
  exportList,
  fence,
  KINDS_SRC,
  list,
  one,
  pkg,
  table,
  TEMPLATE,
  type AuditCase,
} from './fixtures.ts';

export const kinds: AuditCase[] = [
  // Every target kind, matching and drifting, in one report.
  [
    'kinds: list, column and block',
    one(
      KINDS_SRC,
      lines(
        '# Kinds',
        '',
        'Scopes:',
        '',
        '- `read`',
        '- `write`',
        '- `admin`',
        '',
        table(['Method', 'Request path'], ['GET', '`/jobs`'], ['POST', '`/projects`']),
        fence('yaml', TEMPLATE.trim()),
        '',
        fence('', 'docattest   keeps\ndocumentation honest'),
        'Drifting:',
        '',
        '- `read`',
        '- `write`',
        '- `billing`',
        '',
        fence('yaml', 'name: demo\nregion: eu\nport: 8080'),
      ),
    ),
  ],
  // Values copied into sentences: an enumeration of code spans, and single distinctive
  // values, each once per page and never where a fact already reads it.
  [
    'kinds: inline enumerations and scalars',
    one(
      lines(
        "export const LANGUAGES = ['python', 'py', 'pycon'];",
        "export const CORE_VARIABLE = 'DOCATTEST_CORE';",
        "export const CONFIG_FILE = 'docattest.config.json';",
        'export const DEFAULT_PORT = 8080;',
        'export const RETRIES = 3;',
        "export const CURRENCY = 'eur-cents';",
        "export const Money = { unit: 'eur-cents' };",
      ),
      lines(
        '# Inline',
        '',
        "Blocks tagged `python`, `py` or `pycon` are the adapter's. Set",
        '`DOCATTEST_CORE` first, then read `docattest.config.json`.',
        '',
        'Tag a block `python` or `pycon`. The port is 8080, and it tries 3 times.',
        '',
        'Prices are in `eur-cents`. Without `DOCATTEST_CORE` it fails.',
        '',
        'The config is `docattest.config.json`. <!-- fact: CONFIG_FILE -->',
      ),
    ),
  ],
  ['kinds: nothing to suggest', one(KINDS_SRC, '# Prose only\n\nNothing here is copied.\n')],
  [
    'kinds: no pages at all',
    { 'package.json': pkg, 'docattest.config.json': config(), 'src/index.ts': KINDS_SRC },
  ],
  ['kinds: surface without exports', one('export {};\n', list('a1', 'a2'))],
];

export const unitTests: AuditCase[] = [
  // One rule of the matching each, on the smallest project that shows it.
  [
    'unit: arrays, sets, objects, members, row fields',
    one(
      lines(
        "export const ARR = ['a1', 'a2', 'a3'];",
        "export const SET = new Set(['s1', 's2', 's3']);",
        "export const OBJ = { k1: 'v1', k2: 'v2', k3: 'v3' };",
        "export const CONF = { name: 'conf', regions: ['m1', 'm2', 'm3'] };",
        "export const ROWS = [{ id: 'r1', n: 1 }, { id: 'r2', n: 2 }, { id: 'r3' }];",
      ),
      [
        list('a1', 'a2', 'a3'),
        list('s1', 's2', 's3'),
        list('v1', 'v2', 'v3'),
        list('m1', 'm2', 'm3'),
        list('r1', 'r2', 'r3'),
      ].join('\nText.\n\n'),
    ),
  ],
  [
    'unit: functions, instances, types, ragged rows',
    one(
      lines(
        'export class Box { a = "c1"; b = "c2"; }',
        'export const BOX = new Box();',
        "export const make = () => ['f1', 'f2'];",
        'export type T = { t1: string; t2: string };',
        "export const ROWS = [{ n: 'n1' }, { n: 'n2', extra: 'x2' }, { n: 'n3', extra: 'x3' }];",
      ),
      [list('c1', 'c2'), list('f1', 'f2'), list('t1', 't2'), list('x2', 'x3')].join('\nText.\n\n'),
    ),
  ],
  [
    'unit: rows of an object and pairs of fields',
    one(
      lines(
        "export const ROUTES = [{ method: 'GET', path: '/a' }, { method: 'POST', path: '/b' }];",
        'export const ERRORS = { NOT_FOUND: { status: 404 }, DENIED: { status: 403 } };',
      ),
      '| Endpoint | Purpose |\n|---|---|\n| `GET /a` | read |\n| `POST /b` | write |\n\nStatuses:\n\n- `404`\n- `403`\n',
    ),
  ],
  [
    'unit: numeric enum',
    one('export enum Color { Red, Green, Blue }\n', list('Red', 'Green', 'Blue')),
  ],
  [
    'unit: export that throws when read',
    one(
      lines(
        "export const BAD = { get boom() { throw new Error('no'); } };",
        "export const OK = ['o1', 'o2'];",
      ),
      list('o1', 'o2'),
    ),
  ],
  [
    'unit: nested objects and namespaces',
    one(
      lines(
        "export * as Jobs from './jobs.js';",
        "export const schemas = { components: { S3: { properties: { url: {}, bucket: {}, 'allow-write': {} } } } };",
        "export const Strategy = { BASIC: 's1', 'BASIC-EXTEND': 's2' };",
        "export const Deep = { NESTED: { DEEP: ['d1', 'd2'] } };",
      ),
      [
        list('job', 'project', 'owner'),
        list('url', 'bucket'),
        list('s1', 's2'),
        list('d1', 'd2'),
      ].join('\nText.\n\n'),
      {
        'src/jobs.ts':
          'export const CLOSE = { fields: { job: 0, project: 1, owner: 2, approver: 3 } };\n',
      },
    ),
  ],
  [
    'unit: key that is not an identifier',
    one("export const Plans = { 'free-tier': ['p1', 'p2', 'p3'] };\n", list('p1', 'p2', 'p3')),
  ],
  [
    'unit: depth bound and cycles',
    one(
      lines(
        "export const DEEP = { a: { b: { c: { d: { e: { f: { g: ['z1', 'z2'] } } } } } } };",
        "export const LOOP: Record<string, unknown> = { items: ['y1', 'y2'] };",
        'LOOP.self = LOOP;',
      ),
      `${list('z1', 'z2')}\nText.\n\n${list('y1', 'y2')}`,
    ),
  ],
  ['unit: surface throws on load', one('throw new Error("no");\n', list('a1', 'a2'))],
  ['unit: surface imports what is not installed', one("import 'not-installed';\n", 'Prose.\n')],
];

export const suggestions: AuditCase[] = [
  // test/unit/audit/suggest.test.ts and match.test.ts, through real surfaces.
  [
    'unit: earlier of equal values',
    one(
      "export const ErrorCodes = { RPC: 'RPC', SESSION: 'SESSION' };\n",
      '- `RPC`\n- `SESSION`\n',
    ),
  ],
  [
    'unit: closer value wins',
    one(
      lines(exportList('MORE', ['RPC', 'SESSION', 'X1']), exportList('BOTH', ['RPC', 'SESSION'])),
      '- `RPC`\n- `SESSION`\n',
    ),
  ],
  [
    'unit: one column per table, quoted name',
    one(
      "export const ROUTES = [{ method: 'GET', path: '/jobs' }, { method: 'POST', path: '/projects' }, { method: 'PUT', path: '/x' }];\n",
      '| Method | Request path |\n|---|---|\n| GET | `/jobs` |\n| POST | `/projects` |\n',
    ),
  ],
  [
    'unit: deeper set that fits better',
    one(
      lines(
        "export const EventType = { A: 'job', B: 'project', C: 'run' };",
        'export const Jobs = { CLOSE: { fields: { job: 0, project: 1, owner: 2, archive: 3 } } };',
      ),
      '| Name | Note |\n|---|---|\n| `job` | a |\n| `project` | b |\n| `owner` | c |\n',
    ),
  ],
  [
    'unit: unrelated sets tie',
    one(
      lines(
        "export const EventType = { A: 'job', B: 'project', C: 'run' };",
        'export const Jobs = { CLOSE: { fields: { job: 0, project: 1, owner: 2, archive: 3 } }, LIST: { fields: { job: 0, project: 1, owner: 2, run: 3 } } };',
      ),
      '| Name | Note |\n|---|---|\n| `job` | a |\n| `project` | b |\n| `owner` | c |\n',
    ),
  ],
  [
    'unit: candidates with fact item values',
    one(
      lines(
        exportList('KEYS', ['A', 'B', 'plain text']),
        exportList('PATHS', ['/a', '/c']),
        exportList('METHODS', ['GET', 'POST']),
      ),
      lines(
        'Keys:',
        '',
        '- `A` the first',
        '- **B**: a bold lead term',
        '- plain text',
        '',
        '| Method | Path |',
        '|---|---|',
        '| GET | `/a` and `/b` |',
        '| POST | `/c` |',
      ),
    ),
  ],
  [
    'unit: fewer than two distinct items',
    one(
      lines(
        exportList('AS', ['A', 'A2']),
        exportList('NAMES', ['x', 'y']),
        exportList('KINDS', ['same', 'other']),
      ),
      lines(
        '- `A`',
        '- `A`',
        '',
        '| Name | Kind |',
        '|---|---|',
        '| `x` | same |',
        '| `y` | same |',
      ),
    ),
  ],
  [
    'unit: block drift and match',
    one(
      `export const TEMPLATE = ${JSON.stringify(TEMPLATE)};\n`,
      lines(
        fence('yaml', 'name:   demo\n  region: eu\nport: 8080\nlog: info'),
        'Then:',
        '',
        fence('yaml', 'name: demo\nregion: eu\nport: 8080'),
        'And:',
        '',
        fence('yaml', 'name: demo\nregion: eu'),
      ),
    ),
  ],
  [
    'unit: blocks never match short strings, numbers or lists',
    one(
      lines(
        "export const SHORT = 'hello world';",
        'export const BIG = 1234567890123456;',
        exportList('ABC', ['a', 'b', 'c']),
      ),
      lines(
        fence('', 'hello world'),
        'x',
        '',
        fence('', '1234567890123456'),
        'x',
        '',
        fence('', 'a\nb\nc'),
      ),
    ),
  ],
];
