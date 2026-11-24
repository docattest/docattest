import * as path from 'node:path';
import { pathToFileURL } from 'node:url';
import { requireDependency } from '../node/dependency.js';
import { IMPORT_META } from '../run/protocol.js';
import { compileFunction, type CompiledFunction } from './compile.js';
import type { HostClient } from './host-client.js';

/** A CommonJS module record; code may replace `exports`. */
interface ModuleRecord {
  exports: unknown;
}

/**
 * A fresh module registry: one per block, per cases table, per page of facts. Project
 * modules run once per registry, so every block imports the surface fresh and no state
 * leaks between blocks. Dependencies load natively and are shared, as in any Node process.
 */
export class ModuleRegistry {
  private readonly modules = new Map<string, ModuleRecord>();

  constructor(
    private readonly host: HostClient,
    /** Free variables visible to every module this registry runs. */
    private readonly globals: Readonly<Record<string, unknown>>,
  ) {}

  /** `require` as seen from `from`. */
  require(specifier: string, from: string): unknown {
    const resolution = this.host.resolve(specifier, from);
    if (resolution.kind === 'project') return this.load(resolution.file);
    if (resolution.kind === 'external') return requireDependency(specifier, resolution.base);
    throw new Error(resolution.message);
  }

  /** Load a project module by absolute path. */
  load(file: string): unknown {
    const cached = this.modules.get(file);
    if (cached) return cached.exports;
    const source = this.host.source(file);
    if (!source) throw new Error(`Cannot find module ${file}`);
    const record: ModuleRecord = { exports: {} };
    this.modules.set(file, record);
    if (source.format === 'json') record.exports = JSON.parse(source.text) as unknown;
    else this.execute(file, source.code, record, false);
    return record.exports;
  }

  /** Run a block's code as an async module body; settles when the block finishes. */
  async runBlock(file: string, code: string): Promise<void> {
    await this.execute(file, code, { exports: {} }, true);
  }

  private execute(file: string, code: string, record: ModuleRecord, block: boolean): unknown {
    const names = ['exports', 'require', 'module', IMPORT_META, ...Object.keys(this.globals)];
    // Bind __filename and __dirname in an outer closure so CommonJS files can read them while
    // ESM modules declaring `const __dirname = ...` shadow them without SyntaxError, keeping
    // `"use strict"` as a directive prologue and source line numbers unchanged.
    const fn = `${block ? 'async ' : ''}function (${names.join(', ')})`;
    const wrapper = `((__filename, __dirname) => (${fn} {${code}\n}))`;
    const factory = compileFunction(wrapper, file);
    const dirname = path.dirname(file);
    const run = factory(file, dirname) as CompiledFunction;
    const meta = { url: pathToFileURL(file).href, filename: file, dirname };
    const require = (specifier: string) => this.require(specifier, file);
    const args = [record.exports, require, record, meta, ...Object.values(this.globals)];
    // CommonJS runs with `this` as the module's exports; a block is a module body.
    return run.apply(block ? undefined : record.exports, args);
  }
}
