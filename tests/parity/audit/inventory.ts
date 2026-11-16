// `inventory`, and its search.
import { lines } from '../../lib/lines.ts';
import { exportList, KINDS_SRC, list, one, type AuditCase } from './fixtures.ts';

export const inventory: AuditCase[] = [
  // The inventory and its search.
  [
    'inventory: searches',
    one(KINDS_SRC, 'x\n'),
    [
      ['inventory', 'scopes'],
      ['inventory', 'ADMIN'],
      ['inventory', 'nothing-matches-this'],
      ['inventory', 'nothing-matches-this', '--json'],
      ['inventory', ''],
      ['inventory', 'read,write'],
      ['inventory', 'demo'],
      ['inventory', 'ROUTES.map', '--json'],
      ['inventory', 'a', 'b'],
      ['inventory', '--json', 'honest'],
      ['audit', 'ignored', 'words'],
      ['inventory', '--quiet'],
      ['audit', '--surface', 'default'],
      ['inventory', '--nope'],
    ],
  ],
  [
    'inventory: values with newlines, quotes and controls',
    one(
      lines(
        `export const T = ${JSON.stringify('line one\nline "two"\n\ttabbed \\ back\u0001\u007f end')};`,
        exportList('ITEMS', ['a, b', 'c\nd', '', ' spaced ', '"q"']),
        "export const OBJ = { 'a b': 'v 1', 'c-d': 'v,2' };",
      ),
      lines('- a, b', '- `"q"`', '- ` spaced `'),
    ),
  ],
  [
    'inventory: root elsewhere',
    one(KINDS_SRC, list('read', 'write', 'admin')),
    [
      ['audit', '--root', '.'],
      ['inventory', 'scopes', '--root', '__ROOT__'],
      ['audit', '--root', 'nowhere'],
      ['inventory', '--root'],
    ],
  ],
];
