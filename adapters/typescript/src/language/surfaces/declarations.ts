import { ownRelativeTo, type FileSource } from '../../files/source.js';
import { moduleNames, type ModuleName } from '../syntax/module-names.js';
import { parseSource } from '../syntax/parse.js';
import type { ResolvedSurface } from './resolve.js';

const NONE: ReadonlyMap<string, ModuleName> = new Map();

/**
 * The names each surface's module declares at its top level, read from the source of the
 * file it runs from, once per session. Only a module of the project's own is read: a
 * dependency's is what it exports.
 */
export class SurfaceDeclarations {
  private readonly cache = new Map<string, ReadonlyMap<string, ModuleName>>();

  constructor(
    private readonly files: FileSource,
    private readonly surfaces: readonly ResolvedSurface[],
  ) {}

  /** The top-level names of a surface's module; none when it is not the project's. */
  of(surface: string): ReadonlyMap<string, ModuleName> {
    let names = this.cache.get(surface);
    if (!names) {
      names = this.read(surface);
      this.cache.set(surface, names);
    }
    return names;
  }

  private read(surface: string): ReadonlyMap<string, ModuleName> {
    const entry = this.surfaces.find((s) => s.spec.name === surface)?.entry;
    if (entry === undefined || ownRelativeTo(this.files.root, entry) === undefined) return NONE;
    const code = this.files.readAbsolute(entry);
    return code === undefined ? NONE : moduleNames(parseSource(entry, code));
  }
}
