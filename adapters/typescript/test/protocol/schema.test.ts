// The protocol's shapes are published as JSON Schema (protocol/schema), and the package
// ships a copy. The copy is the source, byte for byte; and the conformance suite the core
// carries, which holds every line an adapter writes to those schemas, passes for this adapter.
// The core is the one `scripts/with-core.ts` names in DOCATTEST_CORE.
import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { coreUnderTest } from '../core-under-test.ts';
import { KINDS, STATUSES } from 'docattest';
import { SERVER_COMMAND } from './process.ts';

// A path in this package, and one in the repository around it.
const at = (path: string) => fileURLToPath(new URL(`../../${path}`, import.meta.url));
const repo = (path: string) => fileURLToPath(new URL(`../../../../${path}`, import.meta.url));

describe('the protocol schemas', () => {
  it('ship with the package as they are at their source', () => {
    const names = readdirSync(repo('protocol/schema')).sort();
    expect(names).toEqual([
      'agent.json',
      'attestations.json',
      'config.json',
      'kit.json',
      'language.json',
      'messages.json',
      'methods.json',
      'project.json',
      'results.json',
    ]);
    expect(readdirSync(at('dist/schema')).sort()).toEqual(names);
    for (const name of names) {
      const copy = readFileSync(at(`dist/schema/${name}`), 'utf8');
      expect(copy, name).toBe(readFileSync(repo(`protocol/schema/${name}`), 'utf8'));
    }
  });

  it('own the kinds and statuses a result can carry, which the package exports', () => {
    const types = JSON.parse(readFileSync(repo('protocol/schema/results.json'), 'utf8')) as {
      $defs: { Result: { properties: { kind: { enum: string[] }; status: { enum: string[] } } } };
    };
    const result = types.$defs.Result.properties;
    expect([...KINDS]).toEqual(result.kind.enum);
    expect([...STATUSES]).toEqual(result.status.enum);
  });

  it(
    'are what the TypeScript adapter speaks: the conformance suite holds',
    { timeout: 120_000 },
    () => {
      const core = coreUnderTest();
      const done = spawnSync(
        core,
        ['conformance', 'run', at('conformance.json'), '--', ...SERVER_COMMAND],
        { encoding: 'utf8' },
      );
      expect(done.stdout + done.stderr).toMatch(
        /conformance \(typescript\): (\d+) of \1 checks hold/,
      );
      expect(done.status).toBe(0);
    },
  );
});
