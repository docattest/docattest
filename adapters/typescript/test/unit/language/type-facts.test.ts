import { describe, expect, it } from 'vitest';
import { createProject } from 'docattest';

describe('typeKeys and typeValues facts', () => {
  it('evaluates literal unions and interface keys without loading the module and tracks symbols', async () => {
    const files = {
      'docattest.config.json': JSON.stringify({
        pages: ['README.md'],
        surfaces: { default: './src/index.ts' },
        adapter: 'typescript',
      }),
      'src/index.ts': [
        "export type LogLevel = 'none' | 'error' | 'warn' | 'info' | 'debug';",
        'export interface ClientOptions {',
        '  network?: string;',
        '  rpc?: string;',
        '  logLevel?: LogLevel;',
        '}',
        'export type Extended = ClientOptions & { retries?: number };',
        "export const RUNTIME = 'ok';",
      ].join('\n'),
      'README.md': [
        '# SDK',
        '',
        'Levels: `none`, `error`, `warn`, `info`, `debug`. <!-- fact: typeValues("LogLevel") -->',
        '',
        '<!-- fact column=Option: typeKeys("ClientOptions") -->',
        '| Option | Purpose |',
        '|---|---|',
        '| `network` | cluster |',
        '| `rpc` | endpoint |',
        '| `logLevel` | verbosity |',
        '',
        'Extended keys: `network`, `rpc`, `logLevel`, `retries`. <!-- fact: typeKeys("Extended") -->',
      ].join('\n'),
    };
    const project = createProject({ files });
    const report = await project.check();
    expect(report.results.filter((r) => r.status === 'pass')).toHaveLength(3);
    expect(report.results.filter((r) => r.status !== 'pass')).toHaveLength(0);

    const where = await project.where('LogLevel');
    expect(where).toHaveLength(1);
    expect(where[0]?.page).toBe('README.md');
  });

  it('audits untagged code blocks that import a surface, catching broken examples as drift', async () => {
    const files = {
      'package.json': JSON.stringify({ name: 'acme-sdk' }),
      'docattest.config.json': JSON.stringify({
        pages: ['README.md'],
        surfaces: { default: { import: 'acme-sdk', path: './src/index.ts' } },
        adapter: 'typescript',
      }),
      'src/index.ts': [
        'export function greet(name: string): string { return `hi ${name}`; }',
        "export type Mode = 'fast' | 'safe';",
      ].join('\n'),
      'README.md': [
        '# Acme',
        '',
        'Modes: `fast`, `safe`.',
        '',
        '```ts',
        "import { greet } from 'acme-sdk';",
        "const msg: string = greet('world');",
        '```',
        '',
        '```ts',
        "import { greet } from 'acme-sdk';",
        'greet(123);',
        '```',
      ].join('\n'),
    };
    const project = createProject({ files });
    const report = await project.audit();
    const byTarget = (target: string) => report.suggestions.filter((s) => s.target === target);

    const typeMatch = byTarget('inline');
    expect(typeMatch).toHaveLength(1);
    expect(typeMatch[0]?.expression).toBe("typeValues('Mode')");
    expect(typeMatch[0]?.status).toBe('match');

    const examples = byTarget('example');
    expect(examples).toHaveLength(2);
    expect(examples[0]?.status).toBe('match');
    expect(examples[0]?.directive).toBe('compile');
    expect(examples[1]?.status).toBe('drift');
    expect(examples[1]?.unexpected.join(' ')).toContain('number');
  });
});
