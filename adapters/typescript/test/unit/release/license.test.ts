// npm packs only what is in the package's directory, so the package keeps its own copy of
// the repository's license. It must be the same text.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');

describe('the license the npm package ships', () => {
  it("is the repository's", () => {
    expect(read('../../../LICENSE')).toBe(read('../../../../../LICENSE'));
  });
});
