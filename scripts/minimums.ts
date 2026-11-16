// The oldest toolchains the packages promise a user, read from the manifests that make the
// promise, for the CI job that runs the smoke test on them:
//
//   node scripts/minimums.ts >> "$GITHUB_OUTPUT"
//
// prints `node=…`, `typescript=…` and `python=…`, one per line. The READMEs state the same
// minimums and are held to these manifests by docattest, so docs, manifests and the tested
// toolchains cannot drift apart.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fail } from './lib/fail.ts';
import { readJson, ROOT } from './lib/repository.ts';

/** The fields read from the npm package's package.json. */
interface NpmPackage {
  engines: { node: string };
  peerDependencies: { typescript: string };
}

const npmPackage = readJson('adapters/typescript/package.json') as NpmPackage;
const pyproject = readFileSync(join(ROOT, 'adapters/python/pyproject.toml'), 'utf8');
const minimums = {
  node: lowest(/^>=(\S+)$/, npmPackage.engines.node, 'engines.node'),
  typescript: lowest(/^>=(\S+) </, npmPackage.peerDependencies.typescript, 'typescript'),
  python: lowest(/^requires-python = ">=(\S+)"$/m, pyproject, 'requires-python'),
};
for (const [tool, version] of Object.entries(minimums)) console.log(`${tool}=${version}`);

/** The version `pattern` captures from a requirement; another shape is a failure. */
function lowest(pattern: RegExp, requirement: string, what: string): string {
  return pattern.exec(requirement)?.[1] ?? fail(`${what} is not of the form ${String(pattern)}`);
}
