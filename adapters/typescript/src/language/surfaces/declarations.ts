import { ownRelativeTo, type FileSource } from '../../files/source.js';
import { initializerUses } from '../syntax/initializer-uses.js';
import { moduleNames, type ModuleName } from '../syntax/module-names.js';
import { parseSource } from '../syntax/parse.js';
import type { ResolvedSurface } from './resolve.js';

/** What a surface's module declares at its top level, as far as its source tells. */
interface ModuleDeclarations {
  names: ReadonlyMap<string, ModuleName>;
  /** For each variable, the names its initializer uses ({@link initializerUses}). */
  uses: ReadonlyMap<string, readonly string[]>;
}

const NONE: ModuleDeclarations = { names: new Map(), uses: new Map() };

/**
 * The names each surface's module declares at its top level, read from the source of the
 * file it runs from, once per session. Only a module of the project's own is read: a
 * dependency's is what it exports.
 */
export class SurfaceDeclarations {
  private readonly cache = new Map<string, ModuleDeclarations>();

  constructor(
    private readonly files: FileSource,
    private readonly surfaces: readonly ResolvedSurface[],
  ) {}

  /** The top-level names of a surface's module; none when it is not the project's. */
  of(surface: string): ReadonlyMap<string, ModuleName> {
    return this.module(surface).names;
  }

  /** For each top-level variable of a surface's module, the names its initializer uses. */
  initializerUses(surface: string): ReadonlyMap<string, readonly string[]> {
    return this.module(surface).uses;
  }

  private module(surface: string): ModuleDeclarations {
    let declarations = this.cache.get(surface);
    if (!declarations) {
      declarations = this.read(surface);
      this.cache.set(surface, declarations);
    }
    return declarations;
  }

  private read(surface: string): ModuleDeclarations {
    const entry = this.surfaces.find((s) => s.spec.name === surface)?.entry;
    if (entry === undefined || ownRelativeTo(this.files.root, entry) === undefined) return NONE;
    const code = this.files.readAbsolute(entry);
    if (code === undefined) return NONE;
    const source = parseSource(entry, code);
    return { names: moduleNames(source), uses: initializerUses(source) };
  }
}
