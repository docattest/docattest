import { Buffer } from 'node:buffer';
import type { SymbolMutants, SymbolRef } from '../../api/adapter.js';
import type { FileSource } from '../../files/source.js';
import { surfacePrograms } from '../impact/program.js';
import type { SurfaceSpec } from '../surfaces/specs.js';
import { literalMutants, type KeepsLength } from '../syntax/literal-mutants.js';
import { DeclaredValues, type DeclaredValue } from './declared-value.js';

/**
 * For each symbol, in order, at most `limit` edits that each change one literal of the value
 * it is declared with ({@link literalMutants}), the first in source order; or why it has none.
 * The declarations are read from the code behind the surfaces as `affected` reads it: a
 * surface that points at built output is traced back to its source, the code a change edits.
 */
export function symbolMutants(
  files: FileSource,
  specs: readonly SurfaceSpec[],
  symbols: readonly SymbolRef[],
  limit: number,
): SymbolMutants[] {
  const { program, entries } = surfacePrograms(files, specs);
  const declared = new DeclaredValues(program, entries, files.root);
  const checker = program.getTypeChecker();
  const keepsLength: KeepsLength = (array) => {
    const type = checker.getContextualType(array);
    return type !== undefined && checker.isTupleType(type);
  };
  return symbols.map((symbol) => mutantsOf(declared.of(symbol), limit, keepsLength));
}

function mutantsOf(
  declared: DeclaredValue,
  limit: number,
  keepsLength: KeepsLength,
): SymbolMutants {
  if (declared.kind === 'none') return { edits: [], why: declared.why };
  const { value, source, path } = declared;
  const mutants = literalMutants(value, source, keepsLength).slice(0, limit);
  const [first, ...rest] = mutants.map(({ start, end, replacement, description }) => ({
    path,
    start: utf8Offset(source.text, start),
    end: utf8Offset(source.text, end),
    replacement,
    description,
  }));
  return first ? { edits: [first, ...rest] } : { edits: [], why: 'noLiteral' };
}

// The protocol counts bytes of UTF-8 text; the compiler counts UTF-16 code units.
function utf8Offset(text: string, offset: number): number {
  return Buffer.byteLength(text.slice(0, offset), 'utf8');
}
