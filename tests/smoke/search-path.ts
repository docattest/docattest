// The PATH of an environment the smoke test runs a step in: what is found first, and what
// is not found at all.
import { existsSync } from 'node:fs';
import { delimiter, join } from 'node:path';
import { WINDOWS } from './machine.ts';

/** `env`'s PATH without any directory that holds a `node`, and with `first` in front. */
export function withoutNode(env: NodeJS.ProcessEnv, first: string): NodeJS.ProcessEnv {
  const key = pathKey(env);
  const names = WINDOWS ? ['node.exe', 'node.cmd'] : ['node'];
  const kept = (env[key] ?? '')
    .split(delimiter)
    .filter((dir) => dir !== '' && !names.some((name) => existsSync(join(dir, name))));
  return { ...env, [key]: [first, ...kept].join(delimiter) };
}

/** `env` with `dir` first on its PATH. */
export function pathFirst(env: NodeJS.ProcessEnv, dir: string): NodeJS.ProcessEnv {
  const key = pathKey(env);
  return { ...env, [key]: [dir, env[key]].join(delimiter) };
}

/** The name `env` gives its PATH (Windows spells it `Path`). */
const pathKey = (env: NodeJS.ProcessEnv): string =>
  Object.keys(env).find((name) => name.toUpperCase() === 'PATH') ?? 'PATH';
