// protocol/core.json: the core every package of a release carries, and so the version of
// the release (scripts/version.ts holds every other place to it).
import { readJson, ROOT } from './repository.ts';

/** protocol/core.json: the pin of the core every package carries. */
export interface CorePin {
  version: string;
  protocol: number;
}

/** The version every package of a release carries. */
export const version = (root = ROOT): string =>
  (readJson('protocol/core.json', root) as CorePin).version;
