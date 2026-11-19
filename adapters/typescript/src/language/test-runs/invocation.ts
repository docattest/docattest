/** A runner the project can run its tests with, and the directory it runs them from. */
export type Runner =
  | {
      kind: 'vitest' | 'jest';
      dir: string;
      /** The runner's command, a JavaScript file its package declares as its `bin`. */
      bin: string;
    }
  | { kind: 'node:test'; dir: string };

/** The Node the adapter runs on, which runs every runner. */
export interface NodeRuntime {
  execPath: string;
  /** Whether it strips TypeScript's types by itself (Node 22.18 and later). */
  stripsTypes: boolean;
  /** The URL of the reporter `node --test` writes its report with. */
  reporter: string;
}

/** Some tests of one file to run, and where the runner writes its report. */
export interface RunRequest {
  /** The test file, absolute. */
  file: string;
  /** The full path of each test to select: describe titles, then its title. */
  paths: readonly (readonly string[])[];
  /** Describe titles whose every test is selected: a table whose rows' titles are not known. */
  prefixes: readonly (readonly string[])[];
  reportFile: string;
}

/** A command to run. */
export interface Invocation {
  command: string;
  args: string[];
  cwd: string;
}

/**
 * The command that runs exactly the requested tests of a file and writes a machine-readable
 * report. Each runner selects a test by a regular expression over its full name, its
 * describe titles and title joined by single spaces.
 */
export function invocationOf(runner: Runner, request: RunRequest, node: NodeRuntime): Invocation {
  const { file, reportFile } = request;
  const pattern = namePattern(request.paths, request.prefixes);
  const at = (args: string[]) => ({ command: node.execPath, args, cwd: runner.dir });
  switch (runner.kind) {
    case 'vitest':
      return at([
        runner.bin,
        'run',
        file,
        `--testNamePattern=${pattern}`,
        '--reporter=json',
        `--outputFile=${reportFile}`,
      ]);
    case 'jest':
      return at([
        runner.bin,
        '--json',
        `--outputFile=${reportFile}`,
        `--testNamePattern=${pattern}`,
        '--runTestsByPath',
        file,
      ]);
    case 'node:test':
      return at([
        ...(node.stripsTypes
          ? []
          : ['--experimental-strip-types', '--disable-warning=ExperimentalWarning']),
        '--test',
        `--test-name-pattern=${pattern}`,
        `--test-reporter=${node.reporter}`,
        `--test-reporter-destination=${reportFile}`,
        file,
      ]);
  }
}

/**
 * A regular expression that matches exactly these full names, and every name under these
 * describe titles. Two tests whose titles join to the same name both match it, so the
 * report, not the pattern, tells them apart.
 */
export function namePattern(
  paths: readonly (readonly string[])[],
  prefixes: readonly (readonly string[])[] = [],
): string {
  const exact = paths.map((path) => escapeRegExp(path.join(' ')));
  const under = prefixes.map((prefix) => escapeRegExp(prefix.map((t) => `${t} `).join('')) + '.+');
  const names = new Set([...exact, ...under]);
  return `^(?:${[...names].join('|')})$`;
}

const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
