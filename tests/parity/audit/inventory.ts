// `inventory`, and its search.
import { lines } from '../../lib/lines.ts';
import { exportList, KINDS_SRC, list, one, type AuditCase } from './fixtures.ts';

export const inventory: AuditCase[] = [
  // The inventory and its search.
  [
    'inventory: searches',
    one(KINDS_SRC, 'x\n'),
    [
      ['audit', '--inventory', 'scopes'],
      ['audit', '--inventory', 'ADMIN'],
      ['audit', '--inventory', 'nothing-matches-this'],
      ['audit', '--inventory', 'nothing-matches-this', '--json'],
      ['audit', '--inventory', ''],
      ['audit', '--inventory', 'read,write'],
      ['audit', '--inventory', 'demo'],
      ['audit', '--inventory', 'ROUTES.map', '--json'],
      ['audit', '--inventory', 'a', 'b'],
      ['audit', '--inventory', '--json', 'honest'],
      ['audit', 'ignored', 'words'],
      ['audit', '--inventory', '--quiet'],
      ['audit', '--surface', 'default'],
      ['audit', '--inventory', '--nope'],
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
      ['audit', '--inventory', 'scopes', '--root', '__ROOT__'],
      ['audit', '--root', 'nowhere'],
      ['audit', '--inventory', '--root'],
    ],
  ],
];
