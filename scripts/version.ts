#!/usr/bin/env node
// One version for everything a release ships: the core, its pin, the npm package
// (adapters/typescript) and the packages that carry its core, the Python package, and the
// Go package, whose version is its copy of the pin, the Rust package, and the tag the docs
// pin the GitHub Action to.
//
//   node scripts/version.ts check [--tag v0.1.0]    fail when any of them disagree (or differ
//                                                    from the tag a release is built for)
//   node scripts/version.ts set 0.2.0               write the version into every one of them
//
// `--root <dir>` works on another copy of the repository (the tests use it).
import process from 'node:process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { commandLine } from './lib/command-line.ts';
import type { CorePin } from './lib/core-pin.ts';
import { fail } from './lib/fail.ts';
import { corePackages } from './lib/platforms.ts';
import { ROOT } from './lib/repository.ts';

/** Where in a file a version is written, and the version found there (if any). */
type Found = [where: string, version: string | undefined];

/** One file that holds the version: how to read it, and how to write it. */
interface Place {
  file: string;
  read: (root: string) => Found[];
  write: (root: string, version: string) => void;
}

/** What an npm manifest holds that the version check reads. */
interface NpmManifest {
  version?: string;
  optionalDependencies?: Record<string, string>;
}

/** A package of a package-lock.json: its manifest, where npm fetches it and the hash it checks. */
interface NpmLockEntry extends NpmManifest {
  resolved?: string;
  integrity?: string;
}

/** A package-lock.json: its packages by path. */
interface NpmLock {
  packages?: Record<string, NpmLockEntry>;
}

/** Where npm fetches a package's tarball from. */
const NPM_REGISTRY = 'https://registry.npmjs.org';

/** The npm package is a workspace of the repository's lockfile, at its directory's path. */
const NPM_PACKAGE = 'adapters/typescript';

/** The pin's version, in protocol/core.json and in a package's own copy of it. */
const pin = (file: string): Place =>
  json<CorePin>(
    file,
    (data) => [['version', data.version]],
    (data, v) => {
      data.version = v;
    },
  );

/** Every place the version is written. */
const PLACES: readonly Place[] = [
  text('core/Cargo.toml', /^\[package\]\nname = "docattest-core"\nversion = "([^"]+)"/m),
  text('core/Cargo.lock', /^\[\[package\]\]\nname = "docattest-core"\nversion = "([^"]+)"/m),
  // The core compiles its copy in, so the copy is kept in the tree.
  pin('core/protocol/core.json'),
  text('adapters/rust/Cargo.toml', /^\[package\]\nname = "docattest"\nversion = "([^"]+)"/m),
  // The Rust package (the crate `docattest`) requires exactly its release's core.
  text(
    'adapters/rust/Cargo.toml',
    /^docattest = \{ package = "docattest-core", version = "=([^"]+)"/m,
  ),
  text('adapters/rust/Cargo.lock', /^\[\[package\]\]\nname = "docattest"\nversion = "([^"]+)"/m),
  // The Rust package is built with the core, which its lock file names too.
  text(
    'adapters/rust/Cargo.lock',
    /^\[\[package\]\]\nname = "docattest-core"\nversion = "([^"]+)"/m,
  ),
  text('adapters/python/pyproject.toml', /^\[project\]\nname = "docattest"\nversion = "([^"]+)"/m),
  text('adapters/python/docattest_python/__init__.py', /^__version__ = "([^"]+)"/m),
  pin('protocol/core.json'),
  // The Go package compiles its copy in, so the copy is kept in the tree.
  pin('adapters/go/internal/protocoldata/protocol/core.json'),
  json<NpmManifest>('adapters/typescript/package.json', npmVersions, setNpmVersions),
  json<NpmLock>('package-lock.json', lockVersions, setLockVersions),
  // A workflow pins the GitHub Action (action.yml) to a release's tag.
  text('README.md', /uses: docattest\/docattest@v(\S+)/),
  text('action/README.md', /uses: docattest\/docattest@v(\S+)/),
];

/** A version inside a text file, as the first group of `pattern`. */
function text(file: string, pattern: RegExp): Place {
  return {
    file,
    read: (root) => {
      const match = pattern.exec(readFileSync(join(root, file), 'utf8'));
      return [['version', match?.[1]]];
    },
    write: (root, v) => {
      const path = join(root, file);
      const old = readFileSync(path, 'utf8');
      // With `d`, the match says where the version itself is, so only it is replaced: it may
      // sit inside a longer string (`"=0.1.0"`) or end a line (`@v0.1.0`).
      const at = new RegExp(pattern.source, `${pattern.flags}d`).exec(old)?.indices?.[1];
      if (!at) fail(`${file}: no version where one was expected`);
      writeFileSync(path, old.slice(0, at[0]) + v + old.slice(at[1]));
    },
  };
}

/**
 * Versions inside a JSON file of shape `T` (the repository's own, which this check keeps in
 * shape): `read` lists [where, version] pairs, `update` sets them.
 */
function json<T>(
  file: string,
  read: (data: T, root: string) => Found[],
  update: (data: T, version: string, root: string) => void,
): Place {
  const load = (path: string): T => JSON.parse(readFileSync(path, 'utf8')) as T;
  return {
    file,
    read: (root) => read(load(join(root, file)), root),
    write: (root, v) => {
      const path = join(root, file);
      const data = load(path);
      update(data, v, root);
      writeFileSync(path, `${JSON.stringify(data, null, 2)}\n`);
    },
  };
}

function npmVersions(data: NpmManifest, root: string): Found[] {
  const optional = data.optionalDependencies ?? {};
  const extra = Object.keys(optional).filter((name) => !corePackages(root).includes(name));
  return [
    ['version', data.version],
    ...corePackages(root).map((name): Found => [`optionalDependencies["${name}"]`, optional[name]]),
    ...extra.map((name): Found => [
      `optionalDependencies["${name}"] (not a core package)`,
      undefined,
    ]),
  ];
}

function setNpmVersions(data: NpmManifest, v: string, root: string): void {
  data.version = v;
  data.optionalDependencies = Object.fromEntries(corePackages(root).map((name) => [name, v]));
}

function lockVersions(data: NpmLock, root: string): Found[] {
  const entry = data.packages?.[NPM_PACKAGE] ?? {};
  const workspace = npmVersions(entry, root).map(([where, v]): Found => [
    `packages["${NPM_PACKAGE}"].${where}`,
    v,
  ]);
  const installed = corePackages(root).map((name): Found => {
    const key = `node_modules/${name}`;
    return [`packages["${key}"].version`, data.packages?.[key]?.version];
  });
  return [...workspace, ...installed];
}

function setLockVersions(data: NpmLock, v: string, root: string): void {
  data.packages ??= {};
  data.packages[NPM_PACKAGE] ??= {};
  setNpmVersions(data.packages[NPM_PACKAGE], v, root);
  // A new version's platform packages reach the registry only when its release publishes
  // them, after CI has installed from this lockfile. So the entry names where the tarball
  // will be and no hash: npm ci skips an optional package it cannot fetch, and the first
  // npm install after the release records the hash.
  for (const name of corePackages(root)) {
    const entry = data.packages[`node_modules/${name}`];
    if (entry === undefined) continue;
    entry.version = v;
    entry.resolved = `${NPM_REGISTRY}/${name}/-/${name.split('/').pop() ?? name}-${v}.tgz`;
    delete entry.integrity;
  }
}

/** Every [file, where, version] of the repository at `root`. */
const versions = (root: string): [string, ...Found][] =>
  PLACES.flatMap((place) =>
    place.read(root).map(([where, v]): [string, ...Found] => [place.file, where, v]),
  );

function check(root: string, tag?: string): void {
  const found = versions(root);
  const expected = tag === undefined ? found[0]?.[2] : tag.replace(/^v/, '');
  const wrong = found.filter(([, , v]) => v !== expected);
  if (tag !== undefined && !/^v\d/.test(tag)) fail(`the tag "${tag}" is not v<version>`);
  if (wrong.length > 0) {
    const lines = wrong.map(([file, where, v]) => `  ${file} ${where}: ${v ?? '(missing)'}\n`);
    fail(`versions disagree; expected ${String(expected)} everywhere:\n${lines.join('')}`);
  }
  process.stdout.write(`version ${String(expected)} in all ${found.length} places\n`);
}

const USAGE = 'usage: node scripts/version.ts check [--tag v<version>] | set <version>';
const { values, positionals } = commandLine(
  { allowPositionals: true, options: { tag: { type: 'string' }, root: { type: 'string' } } },
  USAGE,
);
const [command, next, ...more] = positionals;
const root = values.root ?? ROOT;
if (command === 'set' && next !== undefined && more.length === 0) {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(next)) fail(USAGE);
  for (const place of PLACES) place.write(root, next);
  check(root);
} else if (command === 'check' && next === undefined) {
  check(root, values.tag);
} else {
  const stray = command === 'check' ? next : command === 'set' ? more[0] : undefined;
  fail(stray === undefined ? USAGE : `unexpected argument "${stray}"\n${USAGE}`);
}
