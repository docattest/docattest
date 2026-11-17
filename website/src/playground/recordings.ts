/**
 * The playground's recordings, as the recorder (../../recorder/record.ts) wrote them to
 * recordings.json, typed: the compiler holds the generated file to the format both share.
 */
import recorded from './recordings.json' with { type: 'json' };
import type { Recordings } from './recordingFormat.ts';

export const RECORDINGS: Recordings = recorded;
