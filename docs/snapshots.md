---
title: Snapshots
---

# Snapshots

A statement about a system outside the repo cannot be proven, only pinned. A snapshot is
a JSON file in the snapshots directory with a `source`, a `fetchedAt` date and `data`.
Facts and cases reach it through `snapshot('<name>')`. Only `refresh` touches the
network, and it rewrites the file so the change is a reviewable diff.

## A fact can read a snapshot

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, snapshots: { dir: 'docs/.snapshots' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/.snapshots/regions.json': JSON.stringify({ source: 'https://api.example.com/regions', fetchedAt: '2026-10-01', data: [{ code: 'eu' }, { code: 'us' }] }),
    'docs/regions.md': 'Regions: <!-- fact: snapshot("regions").map(r => r.code) -->\n- `eu`\n- `us`\n',
  },
  env: { now: new Date('2026-10-20') },
}).check();

assert.equal(report.results[0].status, 'pass');
assert.equal(report.snapshots.length, 1);
assert.equal(report.snapshots[0].name, 'regions');
assert.equal(report.snapshots[0].ageDays, 19);
assert.equal(report.snapshots[0].warning, false);
```

## A missing snapshot is an error

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, snapshots: { dir: 'docs/.snapshots' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/r.md': 'Regions: <!-- fact: snapshot("regions") -->\n- `eu`\n',
  },
}).check();

assert.equal(report.results[0].status, 'error');
assert.match(report.results[0].message, /regions/);
```

## An old snapshot warns but does not fail

The default age limit is `90` days. <!-- fact: DEFAULT_SNAPSHOT_MAX_AGE_DAYS -->

```ts check
import { createProject } from 'docattest';

const report = await createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, snapshots: { dir: 'docs/.snapshots', maxAgeDays: 30 }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/.snapshots/regions.json': JSON.stringify({ source: 'https://api.example.com/regions', fetchedAt: '2026-01-01', data: [{ code: 'eu' }] }),
    'docs/regions.md': 'Regions: <!-- fact: snapshot("regions").map(r => r.code) -->\n- `eu`\n',
  },
  env: { now: new Date('2026-10-20') },
}).check();

assert.equal(report.results[0].status, 'pass');
assert.equal(report.snapshots[0].warning, true);
assert.equal(report.exitCode, 0);
```

## `refresh` is the only thing that fetches, and it writes the file

`refresh` fetches each snapshot's `source` over HTTP and rewrites its file with the answer
and today's date. It is the docattest core that fetches, so an example or a test serves the
source itself, here on a local port; `env.now` sets the date written.

```ts check
import { createProject } from 'docattest';
import { createServer } from 'node:http';

const server = createServer((_request, response) => response.end(JSON.stringify([{ code: 'eu' }])));
await new Promise<void>((listening) => server.listen(0, '127.0.0.1', listening));
const { port } = server.address() as { port: number };

const project = createProject({
  files: {
    'docattest.config.json': JSON.stringify({ pages: ['docs/**/*.md'], surfaces: { default: './src/index.ts' }, snapshots: { dir: 'docs/.snapshots' }, adapter: 'typescript' }),
    'src/index.ts': `export const x = 1;`,
    'docs/.snapshots/regions.json': JSON.stringify({ source: `http://127.0.0.1:${port}/regions`, fetchedAt: '2026-01-01', data: [] }),
    'docs/regions.md': 'Regions: <!-- fact: snapshot("regions").map(r => r.code) -->\n- `eu`\n',
  },
  env: { now: new Date('2026-10-20') },
});

const before = await project.check();
assert.equal(before.results[0].status, 'fail');

const refreshed = await project.refresh('regions');
server.close();
assert.deepEqual(refreshed, ['regions']);
assert.deepEqual(project.writes(), ['docs/.snapshots/regions.json']);

const after = await project.check();
assert.equal(after.results[0].status, 'pass');
assert.equal(after.snapshots[0].fetchedAt, '2026-10-20');
```
