// Cases whose recordings are the core's own, by design (tests/parity/extra.ts).
import type { ProjectFiles } from '../../lib/project.ts';
import { config, type Run } from './fixtures.ts';

// An adapter for a language neither side has heard of: it greets and answers every request
// with nothing. The core is told where it is as a launcher tells it (PROTOCOL.md §1): a
// JSON array in DOCATTEST_ADAPTER_FAKE, so the path may hold spaces.
const FAKE_ADAPTER = `#!${process.execPath}
const answers = { open: {}, close: {}, symbols: {}, evaluate: {}, describeSurfaces: {}, exports: [] };
const hello = { protocol: 1, adapter: { name: 'fake', version: '0' }, blockLanguages: ['fake'],
  sourceFiles: { include: ['**/*.fake'], exclude: [] }, ignoredNames: ['deps'], capabilities: [] };
require('node:readline').createInterface({ input: process.stdin }).on('line', (line) => {
  const { id, method } = JSON.parse(line);
  console.log(JSON.stringify({ id, result: method === 'hello' ? hello : (answers[method] ?? []) }));
});
`;
const fakeProject = (files: ProjectFiles<string>): ProjectFiles<string> => ({
  'docattest.config.json': config({ adapter: 'fake' }),
  'my tools/.bin/docattest-adapter-fake': FAKE_ADAPTER,
  ...files,
});

// Cases where the core does by design what the reference refused (tests/parity/recordings.ts
// says what the reference was): the core drives any adapter program through the protocol.
// Their recordings are the core's own, and each must also show the behaviour stated in
// `candidate`, which is checked again whenever one is recorded.
export interface DifferingCase {
  name: string;
  files: ProjectFiles<string>;
  args: readonly string[];
  /** The behaviour the core was designed to have here. */
  candidate: (out: Run) => boolean;
}

export const differing: DifferingCase[] = [
  {
    name: 'an installed adapter other than typescript: check',
    files: fakeProject({ 'docs/a.md': '# A\n\nProse only.\n', 'deps/docs/b.md': 'never a page\n' }),
    args: ['check'],
    candidate: (out) =>
      out.code === 0 &&
      out.stdout.includes('coverage: 1 living page') &&
      out.stdout.trimEnd().endsWith('ok') &&
      out.stderr === '',
  },
  {
    name: 'an installed adapter other than typescript: a block in its language',
    files: fakeProject({ 'docs/a.md': '```fake chekc\nx\n```\n\n```ts chekc\nx\n```\n' }),
    args: ['check'],
    candidate: (out) =>
      out.code === 1 && out.stdout.includes('unknown tag "chekc"') && !out.stdout.includes('L5'),
  },
];
