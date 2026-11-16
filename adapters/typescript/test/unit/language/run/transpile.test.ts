import { describe, expect, it } from 'vitest';
import { IMPORT_META } from '../../../../src/language/run/protocol.js';
import { decodeLines, decodeVlq } from '../../../../src/language/run/sourcemap.js';
import { transpile } from '../../../../src/language/run/transpile.js';

describe('source map lines', () => {
  it('decodes VLQ fields', () => {
    expect(decodeVlq('AAAA')).toEqual([0, 0, 0, 0]);
    expect(decodeVlq('AACA')).toEqual([0, 0, 1, 0]);
    expect(decodeVlq('AAAD')).toEqual([0, 0, 0, -1]);
    expect(decodeVlq('gBAAA')).toEqual([16, 0, 0, 0]);
  });

  it('maps each generated line to the first source line it starts', () => {
    expect(decodeLines('AAAA;;AACA,AAEA;AACA')).toEqual([0, undefined, 1, 4]);
  });
});

describe('transpile', () => {
  it('emits CommonJS with a line map back to the source', () => {
    const { code, lines } = transpile(
      '/x/a.ts',
      "import { b } from './b.js';\n\nconst n: number = b;\nexport { n };\n",
    );
    expect(code).toMatch(/require\("\.\/b\.js"\)/);
    const generated = code.split('\n').findIndex((l) => l.includes('const n'));
    expect(lines[generated]).toBe(2);
  });

  it('turns import.meta into the free variable the registry provides', () => {
    const { code } = transpile('/x/m.ts', 'export const here = new URL(".", import.meta.url);');
    expect(code).toContain(`${IMPORT_META}.url`);
    expect(code).not.toContain('import.meta');
  });

  it('is cached by path and text', () => {
    expect(transpile('/x/c.ts', 'export {};')).toBe(transpile('/x/c.ts', 'export {};'));
  });
});
