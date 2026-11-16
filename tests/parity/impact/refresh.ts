// `refresh`, against the local server (server.ts).
import { snapshot, withSnapshots } from './fixtures.ts';
import type { ImpactCase } from './types.ts';
import { DEAD, URL_BASE } from './server.ts';

export const refresh: ImpactCase[] = [
  // ------------------------------------------------------------------------------ refresh
  {
    name: 'refresh every snapshot',
    files: withSnapshots({
      'docs/.snapshots/regions.json': snapshot('/regions'),
      'docs/.snapshots/order.json': snapshot('/order'),
      'docs/.snapshots/bom.json': snapshot('/bom'),
      'docs/.snapshots/scalar.json': snapshot('/scalar'),
      'docs/.snapshots/zip.json': snapshot('/gzip'),
      'docs/.snapshots/moved.json': snapshot('/moved'),
      'docs/.snapshots/agent.json': snapshot('/agent'),
      'docs/.snapshots/surrogate.json': snapshot('/surrogate'),
      'docs/.snapshots/notes.txt': 'kept\n',
    }),
    steps: [['refresh'], ['check']],
  },
  {
    name: 'refresh one snapshot by name',
    files: withSnapshots({
      'docs/.snapshots/regions.json': snapshot('/regions'),
      'docs/.snapshots/order.json': snapshot('/order'),
    }),
    steps: [
      ['refresh', 'order'],
      ['refresh', 'regions', 'ignored'],
      ['refresh', 'nope'],
      ['refresh', ''],
    ],
  },
  {
    name: 'refresh keeps a query and a fragment in the source',
    files: withSnapshots({ 'docs/.snapshots/q.json': snapshot('/regions?a=1&b=%20x#frag') }),
    steps: [['refresh']],
  },
  {
    name: 'refresh skips broken snapshot files',
    files: withSnapshots({
      'docs/.snapshots/bad.json': '{',
      'docs/.snapshots/shape.json': '{"source":1}',
      'docs/.snapshots/date.json': `{"source":"${URL_BASE}/regions","fetchedAt":"nope","data":1}`,
      'docs/.snapshots/ok.json': snapshot('/regions', '2019-12-31T23:00:00Z'),
    }),
    steps: [['refresh'], ['refresh', 'bad']],
  },
  {
    name: 'refresh with nothing to refresh',
    files: withSnapshots({}),
    steps: [['refresh'], ['refresh', 'x']],
  },
  {
    name: 'refresh without a snapshots directory',
    files: withSnapshots({}, {}),
    steps: [['refresh'], ['refresh', 'x']],
  },
  {
    name: 'refresh a source that answers 404',
    files: withSnapshots({
      'docs/.snapshots/a.json': snapshot('/regions'),
      'docs/.snapshots/b.json': snapshot('/missing'),
      'docs/.snapshots/c.json': snapshot('/order'),
    }),
    steps: [['refresh'], ['refresh', 'c']],
  },
  {
    name: 'refresh a source that answers 500',
    files: withSnapshots({ 'docs/.snapshots/b.json': snapshot('/broken') }),
    steps: [['refresh']],
  },
  {
    name: 'refresh a source that answers 204',
    files: withSnapshots({ 'docs/.snapshots/b.json': snapshot('/nothing') }),
    steps: [['refresh']],
  },
  {
    name: 'refresh a body that is not JSON',
    files: withSnapshots({
      'docs/.snapshots/a.json': snapshot('/regions'),
      'docs/.snapshots/h.json': snapshot('/html'),
    }),
    steps: [['refresh']],
  },
  {
    name: 'refresh an empty body',
    files: withSnapshots({ 'docs/.snapshots/e.json': snapshot('/empty') }),
    steps: [['refresh']],
  },
  {
    name: 'refresh a redirect that never ends',
    files: withSnapshots({ 'docs/.snapshots/l.json': snapshot('/loop') }),
    steps: [['refresh']],
  },
  {
    name: 'refresh a source nothing listens on',
    files: withSnapshots({
      'docs/.snapshots/d.json': JSON.stringify({
        source: `${DEAD}/x`,
        fetchedAt: '2020-01-01',
        data: 1,
      }),
    }),
    steps: [['refresh']],
  },
  {
    name: 'refresh a source that is not a URL',
    files: withSnapshots({
      'docs/.snapshots/d.json': JSON.stringify({
        source: 'not a url',
        fetchedAt: '2020-01-01',
        data: 1,
      }),
    }),
    steps: [['refresh']],
  },
  {
    name: 'refresh a source with another scheme',
    files: withSnapshots({
      'docs/.snapshots/d.json': JSON.stringify({
        source: 'ftp://127.0.0.1/x.json',
        fetchedAt: '2020-01-01',
        data: 1,
      }),
    }),
    steps: [['refresh']],
  },
  { name: 'refresh without a config', files: { 'README.md': 'x\n' }, steps: [['refresh']] },
];
