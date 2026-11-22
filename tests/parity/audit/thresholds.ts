// Ties, thresholds at their boundaries, and the adapter's walk at its bounds.
import { lines } from '../../lib/lines.ts';
import {
  AUDIT,
  exportList,
  fence,
  list,
  nest,
  one,
  range,
  table,
  TEMPLATE,
  type AuditCase,
} from './fixtures.ts';

export const ties: AuditCase[] = [
  // Ties.
  [
    'ties: equal and disagreeing',
    one(
      lines(exportList('X', ['aa', 'bb', 'cc']), exportList('Y', ['aa', 'bb', 'dd'])),
      '- `aa`\n- `bb`\n',
    ),
  ],
  [
    'ties: equal and agreeing',
    one(
      lines(exportList('X', ['aa', 'bb', 'cc']), exportList('Y', ['cc', 'bb', 'aa'])),
      '- `aa`\n- `bb`\n',
    ),
  ],
  [
    'ties: broken by a later stronger value',
    one(
      lines(
        exportList('X', ['aa', 'bb', 'cc']),
        exportList('Y', ['aa', 'bb', 'dd']),
        exportList('Z', ['bb', 'aa']),
      ),
      '- `aa`\n- `bb`\n',
    ),
  ],
  [
    'ties: tie after the best',
    one(
      lines(
        exportList('Z', ['bb', 'aa']),
        exportList('X', ['aa', 'bb', 'cc']),
        exportList('Y', ['aa', 'bb', 'dd']),
      ),
      '- `aa`\n- `bb`\n',
    ),
  ],
  [
    'ties: columns of one table compete',
    one(
      lines(
        exportList('IDS', ['id1', 'id2', 'id3']),
        exportList('NAMES', ['alpha', 'beta', 'gamma']),
      ),
      table(['Id', 'Name'], ['`id1`', '`alpha`'], ['`id2`', '`beta`'], ['`id3`', '`gamma`']),
    ),
  ],
  [
    'ties: same score, heavier column wins',
    one(
      lines(exportList('IDS', ['i1', 'i2']), exportList('NAMES', ['alphabet', 'betatron'])),
      table(['Name', 'Id'], ['`alphabet`', '`i1`'], ['`betatron`', '`i2`']),
    ),
  ],
  [
    'ties: blocks that equal two strings',
    one(
      lines(
        `export const A = ${JSON.stringify(TEMPLATE)};`,
        `export const B = ${JSON.stringify(TEMPLATE)};`,
      ),
      fence('yaml', TEMPLATE.trim()),
    ),
  ],
  [
    'ties: block drifting from two different strings',
    one(
      lines(
        `export const A = ${JSON.stringify(TEMPLATE + 'tail: one\n')};`,
        `export const B = ${JSON.stringify(TEMPLATE + 'tail: two\n')};`,
      ),
      fence('yaml', TEMPLATE.trim()),
    ),
  ],
];

export const thresholds: AuditCase[] = [
  // Thresholds at their boundaries.
  [
    'threshold: overlap exactly one half',
    one(exportList('K', ['a1', 'a2', 'a3', 'a4', 'a5', 'a6']) + '\n', list('a1', 'a2', 'a3')),
  ],
  [
    'threshold: overlap just under one half',
    one(exportList('K', ['a1', 'a2', 'a3', 'a4', 'a5', 'a6', 'a7']) + '\n', list('a1', 'a2', 'a3')),
  ],
  [
    'threshold: half with differences both sides',
    one(
      exportList('K', ['a1', 'a2', 'a3', 'a4', 'b1']) + '\n',
      list('a1', 'a2', 'a3', 'a4', 'c1', 'c2', 'c3'),
    ),
  ],
  [
    'threshold: two shared, one difference',
    one(exportList('K', ['read', 'write', 'admin']) + '\n', list('read', 'write')),
  ],
  [
    'threshold: two shared, two differences',
    one(exportList('K', ['read', 'write', 'admin']) + '\n', list('read', 'write', 'billing')),
  ],
  [
    'threshold: three shared, two differences',
    one(
      exportList('K', ['read', 'write', 'exec', 'admin']) + '\n',
      list('read', 'write', 'exec', 'billing'),
    ),
  ],
  [
    'threshold: trivial items',
    one(
      lines(
        exportList('DIGITS', ['1', '2', 'x']),
        exportList('WORDS', ['yes', 'no', 'on']),
        exportList('PORTS', ['80', '443']),
        exportList('MIXED', ['TRUE', 'None', 'real', 'other']),
        exportList('NEG', ['-1', '-2', '-10', '-20']),
        exportList('ONE', ['alpha']),
      ),
      [
        list('1', '2', 'x'),
        list('yes', 'no', 'on'),
        list('80', '443'),
        list('TRUE', 'None', 'real', 'other'),
        list('TRUE', 'None', 'real'),
        list('-1', '-2'),
        list('-10', '-20'),
        list('alpha'),
      ].join('\nText.\n\n'),
    ),
  ],
  [
    'threshold: duplicates and empty items',
    one(
      exportList('K', ['eu', 'us', 'eu']) + '\n',
      lines(
        '- `eu`',
        '-',
        '- `us`',
        '- `eu`',
        '',
        '| R | Note |',
        '|---|---|',
        '| `eu` | |',
        '| | |',
        '| `us` | |',
      ),
    ),
  ],
  [
    'threshold: block of sixteen characters',
    one(
      lines("export const S16 = 'abcdefgh ijklmno';", "export const S15 = 'abcdefg  hijklmn';"),
      lines(fence('', 'abcdefgh\nijklmno'), 'x', '', fence('', 'abcdefg hijklmn')),
    ),
  ],
  [
    'threshold: block lines of three characters',
    one(
      `export const T = ${JSON.stringify('abc\nde\n---\nx y\nlonger line\nlast line here\n')};\n`,
      lines(
        fence('', 'abc\nde\n---\nx y\nlonger line'),
        'x',
        '',
        fence('', 'abc\nx y\nother line'),
      ),
    ),
  ],
  [
    'threshold: block with two lines each side',
    one(
      `export const T = ${JSON.stringify('first line here\nsecond line here\n')};\n`,
      fence('', 'first line here\nsecond line there'),
    ),
  ],
];

export const walk: AuditCase[] = [
  // The adapter's walk at its bounds (MAX_DEPTH 6, MAX_MEMBERS 100, MAX_ITEMS 500, MAX_NODES 400).
  ...[5, 6, 7].map((depth): AuditCase => [
    `walk: list ${depth} levels down`,
    one(`export const DEEP = ${nest(depth, "['z1', 'z2', 'z3']")};\n`, list('z1', 'z2', 'z3')),
  ]),
  ...[5, 6, 7].map((depth): AuditCase => [
    `walk: keys ${depth} levels down`,
    one(`export const DEEP = ${nest(depth, '{ k1: 1, k2: 2, k3: 3 }')};\n`, list('k1', 'k2', 'k3')),
  ]),
  ...[100, 101].map((n): AuditCase => [
    `walk: member ${n} of an object`,
    one(
      `export const WIDE = { ${range(n - 1, (k) => `m${k}: ${k}`).join(', ')}, last: ['w1', 'w2', 'w3'] };\n`,
      list('w1', 'w2', 'w3'),
    ),
    [
      ['audit'],
      ['audit', '--json'],
      ['audit', '--inventory', 'last'],
      ['audit', '--inventory', 'w1', '--json'],
    ],
  ]),
  ...[500, 501].map((n): AuditCase => [
    `walk: list of ${n} items`,
    one(
      `export const LONG = ${JSON.stringify(range(n, (k) => `item${k}`))};\n`,
      range(n, (k) => `- \`item${k}\``).join('\n') + '\n',
    ),
    [
      ['audit'],
      ['audit', '--json'],
      ['audit', '--inventory', 'item499'],
      ['audit', '--inventory', 'item500', '--json'],
    ],
  ]),
  ...[500, 501].map((n): AuditCase => [
    `walk: object of ${n} keys`,
    one(
      `export const BIG = { ${range(n, (k) => `key${k}: 'v${k}'`).join(', ')} };\n`,
      list('key1', 'key2', 'key3') +
        '\nText.\n\n' +
        range(n, (k) => `- \`v${k}\``).join('\n') +
        '\n',
    ),
    AUDIT,
  ]),
  [
    'walk: more nodes than the bound',
    one(
      `export const MANY = { ${range(60, (a) => `g${a}: { ${range(8, (b) => `h${b}: ['n${a}x${b}a', 'n${a}x${b}b']`).join(', ')} }`).join(', ')} };\n`,
      [
        list('n0x0a', 'n0x0b'),
        list('n40x7a', 'n40x7b'),
        list('n48x0a', 'n48x0b'),
        list('n59x7a', 'n59x7b'),
      ].join('\nText.\n\n'),
    ),
  ],
  [
    'walk: rows with many fields',
    one(
      `export const ROWS = [${range(3, (r) => `{ ${range(8, (f) => `f${f}: 'r${r}f${f}'`).join(', ')} }`).join(', ')}];\n`,
      table(
        ['A', 'B'],
        ['`r0f0 r0f1`', '`r0f7`'],
        ['`r1f0 r1f1`', '`r1f7`'],
        ['`r2f0 r2f1`', '`r2f7`'],
      ),
    ),
  ],
  [
    'walk: maps, sets, dates, symbols and nulls',
    one(
      lines(
        "export const MAP = new Map([['mk1', 'mv1'], ['mk2', 'mv2']]);",
        "export const SET = new Set(['s1', 's2']);",
        'export const MIX = [1, "two", null, undefined, true];',
        'export const NUL = null;',
        'export const UND = undefined;',
        'export const WHEN = new Date(0);',
        "export const SYM = Symbol('sym');",
        'export const BIG = 12345678901234567890n;',
        'export const NUM = 1e21;',
        'export const NEG0 = -0;',
        "export const NESTED = [['in1', 'in2'], ['in3']];",
        "export default ['dflt1', 'dflt2'];",
        "export const $dollar = ['dl1', 'dl2'];",
        'export const EMPTY: string[] = [];',
        'export const EMPTY_OBJ = {};',
      ),
      [
        list('mk1', 'mk2'),
        list('mv1', 'mv2'),
        list('s1', 's2'),
        list('two', 'true', 'null'),
        list('dflt1', 'dflt2'),
        list('dl1', 'dl2'),
      ].join('\nText.\n\n'),
    ),
  ],
];
