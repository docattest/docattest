// One test per automatable criterion in ACCEPTANCE.md. Titles carry the criterion number
// and the docs page that states the behaviour in prose.
import { describe, it, expect } from 'vitest';
import { project } from './project.ts';

const N3 = `export const N = 3;\nexport const add = (a: number, b: number) => a + b;`;

describe('detection', () => {
  it('A1 stale fact fails with file, line and discrepancy (docs/facts.md)', async () => {
    const r = await project({
      'src/index.ts': N3,
      'docs/a.md': 'Intro.\n\nN is `4`. <!-- fact: N -->',
    }).check();
    expect(r.results[0]).toMatchObject({
      page: 'docs/a.md',
      line: 3,
      status: 'fail',
      expected: '4',
      actual: '3',
    });
    expect(r.exitCode).toBe(1);
  });
  it('A2 stale behaviour fails at the doc location (docs/examples.md)', async () => {
    const r = await project({
      'src/index.ts': N3,
      'docs/a.md':
        "Adds.\n\n```ts check\nimport { add } from 'docattest-fixture';\nassert.equal(add(1, 1), 3);\n```",
    }).check();
    expect(r.results[0]).toMatchObject({ page: 'docs/a.md', line: 3, status: 'fail' });
  });
  it('A3 a sample that does not compile fails (docs/examples.md)', async () => {
    const r = await project({
      'src/index.ts': N3,
      'docs/a.md': "```ts compile\nimport { nope } from 'docattest-fixture';\n```",
    }).check();
    expect(r.results[0].status).toBe('fail');
    expect(r.results[0].message).toMatch(/nope/);
  });
  it('A5 doc comments are checked like pages (docs/pages.md)', async () => {
    const src =
      "/**\n * @example\n * // check\n * import { add } from 'docattest-fixture';\n * assert.equal(add(1, 1), 3);\n */\nexport const add = (a: number, b: number) => a + b;";
    const files = {
      'docattest.config.json': JSON.stringify({
        pages: [],
        sources: ['src/**/*.ts'],
        surfaces: { default: './src/index.ts' },
        adapter: 'typescript',
      }),
      'src/index.ts': src,
    };
    const r = await project(files).check();
    expect(r.results[0]).toMatchObject({ page: 'src/index.ts', status: 'fail', symbols: ['add'] });
  });
});

describe('noise', () => {
  it('A6 a refactor that keeps behaviour produces the same report (docs/report.md)', async () => {
    const page = {
      'docs/a.md':
        "N is `3`. <!-- fact: N -->\n\n```ts check\nimport { add } from 'docattest-fixture';\nassert.equal(add(2, 2), 4);\n```",
    };
    const before = await project({ 'src/index.ts': N3, ...page }).check();
    const refactored = `const base = 3;\nexport const N = base;\nexport function add(a: number, b: number) { return b + a; }`;
    const after = await project({ 'src/index.ts': refactored, ...page }).check();
    // A refactor may change the names a claim rests on (N is now built from `base`), not
    // what the report says of the claim.
    const claims = (r: typeof after) => r.results.map((claim) => ({ ...claim, symbols: [] }));
    expect({ ...after, results: claims(after) }).toEqual({ ...before, results: claims(before) });
    expect(after.exitCode).toBe(0);
  });
  it('A7 a failure carries expected, actual and line; nothing else to acknowledge (docs/report.md)', async () => {
    const p = project({ 'src/index.ts': N3, 'docs/a.md': 'N is `4`. <!-- fact: N -->' });
    const r = await p.check();
    expect(r.results[0]).toMatchObject({ line: 1, expected: '4', actual: '3' });
    expect(p.writes()).toEqual([]);
  });
});

describe('authoring and scope', () => {
  it('A9 uncovered pages are reported, not failed (docs/pages.md)', async () => {
    const r = await project({ 'src/index.ts': N3, 'docs/a.md': 'Prose only.' }).check();
    expect(r.coverage.uncovered).toEqual(['docs/a.md']);
    expect(r.exitCode).toBe(0);
  });
  it('A10 a check writes nothing (docs/report.md)', async () => {
    const p = project({ 'src/index.ts': N3, 'docs/a.md': 'N is `3`. <!-- fact: N -->' });
    await p.check();
    expect(p.writes()).toEqual([]);
  });
  it('A11 frozen pages are excluded by name (docs/pages.md)', async () => {
    const files = {
      'docattest.config.json': JSON.stringify({
        pages: ['docs/**/*.md'],
        frozen: ['docs/old/**'],
        surfaces: { default: './src/index.ts' },
        adapter: 'typescript',
      }),
      'src/index.ts': N3,
      'docs/old/a.md': 'N is `9`. <!-- fact: N -->',
    };
    const r = await project(files).check();
    expect(r.results).toHaveLength(0);
    expect(r.coverage.frozen).toEqual(['docs/old/a.md']);
  });
});

describe('duality', () => {
  it('A12 a symbol maps to pages, lines and claims; unmentioned symbols are listed (docs/duality.md)', async () => {
    const p = project({
      'src/index.ts': N3,
      'docs/a.md': 'Three is the limit.\n\nN is `3`. <!-- fact: N -->',
    });
    expect(await p.where('N')).toEqual([
      { page: 'docs/a.md', line: 3, kind: 'fact', claim: 'N is `3`.' },
    ]);
    expect(await p.uncovered()).toEqual(['add']);
  });
});

describe('operations', () => {
  it('A14 deterministic: same inputs, same report (docs/report.md)', async () => {
    const files = { 'src/index.ts': N3, 'docs/a.md': 'N is `4`. <!-- fact: N -->' };
    expect(await project(files).check()).toEqual(await project(files).check());
  });
  it('A14 offline: an example that reaches the network fails instead of hanging (docs/examples.md)', async () => {
    const r = await project(
      { 'src/index.ts': N3, 'docs/a.md': "```ts check\nawait fetch('https://example.com');\n```" },
      { network: false },
    ).check();
    expect(r.results[0].status).toBe('fail');
    expect(r.results[0].message).toMatch(/network/i);
  });
});
