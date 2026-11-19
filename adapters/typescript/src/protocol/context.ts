import type { AdapterContext, ConfiguredSurface, SourceChange } from '../api/adapter.js';
import { diskFiles } from '../files/disk.js';
import type { Ignored } from '../files/ignored.js';
import { memoryFiles } from '../files/memory.js';
import type { FileSource } from '../files/source.js';
import { overlay } from './overlay.js';
import { isObject } from '../json.js';
import { Params, ProtocolError } from './wire.js';

/**
 * The adapter context `open` describes. With `files` the project lives in memory and `root`
 * is not read; without, it is the directory at `root`, less what the language ignores.
 */
export function contextOf(params: Params, ignored: Ignored): AdapterContext {
  const files = params.has('files')
    ? memoryFiles(params.textMap('files'))
    : diskFiles(params.string('root'), ignored);
  const snapshots = new Params(
    'open: snapshots',
    params.has('snapshots') ? params.object('snapshots') : {},
  );
  return {
    files,
    surfaces: params.list('surfaces').map(surfaceOf),
    setup: params.optionalString('setup'),
    timeoutMs: params.number('timeoutMs'),
    workers: params.number('workers'),
    network: params.boolean('network'),
    snapshots: {
      dir: snapshots.optionalString('dir'),
      data: new Map(Object.entries(snapshots.has('data') ? snapshots.object('data') : {})),
    },
    manifests: new Map(Object.entries(params.has('manifests') ? params.object('manifests') : {})),
    texts: new Map(Object.entries(params.has('texts') ? params.textMap('texts') : {})),
    matches: new Map(Object.entries(params.has('matches') ? params.textMaps('matches') : {})),
    manifestFields: new Map(
      Object.entries(params.has('manifestFields') ? params.objects('manifestFields') : {}),
    ),
    examples: params.has('examples') ? params.attestations('examples', 'example') : [],
  };
}

// A surface as the config states it: `{ name, entry }`, the entry a string or `{ import, path }`.
function surfaceOf(value: unknown): ConfiguredSurface {
  const { name, entry } = isObject(value) ? value : {};
  const both =
    isObject(entry) && typeof entry.import === 'string' && typeof entry.path === 'string';
  if (typeof name !== 'string' || !(typeof entry === 'string' || both)) {
    throw new ProtocolError('open: each of "surfaces" must have a "name" and an "entry"');
  }
  return { name, entry } as ConfiguredSurface;
}

/**
 * The change `affected` describes: the project as it is now, with each path in `base`
 * replaced by its base contents (null: it did not exist then).
 */
export function changeOf(files: FileSource, params: Params): SourceChange {
  const base = params.textOrNullMap('base');
  const read = (file: string) => base[file] ?? undefined;
  return { base: overlay(files, Object.keys(base), read), changed: params.strings('changed') };
}
