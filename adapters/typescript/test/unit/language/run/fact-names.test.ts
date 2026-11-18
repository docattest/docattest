import { describe, expect, it } from 'vitest';
import { bindFactNames } from '../../../../src/language/run/fact-names.js';
import { moduleNames } from '../../../../src/language/syntax/module-names.js';
import { parseSource } from '../../../../src/language/syntax/parse.js';

const scope = {
  surface: 'default',
  exported: new Set(['LIMIT', 'retry']),
  helpers: new Set(['errorMessages', 'PORT']),
  declared: moduleNames(
    parseSource(
      '/m.ts',
      'export const LIMIT = 3;\nconst PORT = 8080;\nconst SECRET = 1;\nexport function retry() {}',
    ),
  ),
};

describe('bindFactNames', () => {
  it('reads constants from source, and loads the module for anything else it names', () => {
    expect(bindFactNames(['LIMIT', 'Math'], scope)).toEqual({
      ok: true,
      constants: [['LIMIT', 3]],
      loads: false,
    });
    expect(bindFactNames(['SECRET', 'manifest'], scope)).toEqual({
      ok: true,
      constants: [['SECRET', 1]],
      loads: false,
    });
    expect(bindFactNames(['retry'], scope)).toEqual({ ok: true, constants: [], loads: true });
    expect(bindFactNames(['errorMessages'], scope)).toEqual({
      ok: true,
      constants: [],
      loads: true,
    });
  });

  it('refuses a constant the setup module also names as a fact', () => {
    expect(bindFactNames(['PORT'], scope)).toEqual({
      ok: false,
      message:
        '"PORT" is both a constant of surface "default" and a setup fact; rename the setup fact',
    });
  });
});
