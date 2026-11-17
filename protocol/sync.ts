#!/usr/bin/env node
// Copy the protocol's neutral files (the schemas, core.json, platforms.json, and the agent
// descriptors and texts) into a package being built:
//
//   node protocol/sync.ts <directory>       writes <directory>/schema/*.json, core.json,
//                                           platforms.json and <directory>/agents/*
//
// Each language package ships its own copy, made here from the one source and never by
// hand; a test in each package fails when its copy differs.
import { cpSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const source = dirname(fileURLToPath(import.meta.url));
const [target] = process.argv.slice(2);
if (!target) throw new Error('usage: node protocol/sync.ts <directory>');
const into = resolve(target);
// The JSON files, and the agent texts beside the descriptors (a directory's README is for
// people reading this repository, and is not copied).
const copied = (dir: string): string[] =>
  readdirSync(dir).filter((f) => f.endsWith('.json') || (f.endsWith('.md') && f !== 'README.md'));
// A directory is replaced whole, except for what the package compiled into it itself.
for (const dir of ['schema', 'agents']) {
  mkdirSync(join(into, dir), { recursive: true });
  for (const old of copied(join(into, dir))) rmSync(join(into, dir, old));
  for (const file of copied(join(source, dir))) {
    cpSync(join(source, dir, file), join(into, dir, file));
  }
}
for (const file of ['core.json', 'platforms.json']) cpSync(join(source, file), join(into, file));
