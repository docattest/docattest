import ts from '../ts.cjs';
import type { ExampleAttestation } from '../../api/attestation.js';
import { prose } from './prose.js';
import { asserts } from './lexical.js';
import { parseSource } from './parse.js';

interface CommentExample {
  mode: 'check' | 'compile';
  network: boolean;
  code: string;
  claim: string;
}

/** `@example` blocks in doc comments whose first line is `// check` or `// compile`. */
export function docCommentExamples(page: string, text: string): ExampleAttestation[] {
  const source = parseSource(page, text);
  const out: ExampleAttestation[] = [];
  const seen = new Set<number>();
  for (const statement of source.statements) {
    for (const range of ts.getLeadingCommentRanges(text, statement.pos) ?? []) {
      const comment = text.slice(range.pos, range.end);
      if (!comment.startsWith('/**') || seen.has(range.pos)) continue;
      seen.add(range.pos);
      const line = source.getLineAndCharacterOfPosition(range.pos).line + 1;
      const owner = exportedName(statement);
      for (const ex of examplesIn(comment)) {
        const id = `${page}:${line}:example:${out.length}`;
        const weak = ex.mode === 'check' && !asserts(ex.code);
        const { mode, network, code, claim } = ex;
        const example: ExampleAttestation = {
          id,
          page,
          line,
          kind: 'example',
          claim,
          lang: 'ts',
          mode,
          network,
          code,
          weak,
        };
        out.push(owner === undefined ? example : { ...example, owner });
      }
    }
  }
  return out;
}

function examplesIn(comment: string): CommentExample[] {
  const lines = comment
    .replace(/^\/\*\*/, '')
    .replace(/\*\/$/, '')
    .split('\n')
    .map((l) => l.replace(/^\s*\* ?/, ''));
  const firstTag = lines.findIndex((l) => /^\s*@\w/.test(l));
  const claim = prose(lines.slice(0, Math.max(0, firstTag)).join(' '));
  return lines.flatMap((l, k) => {
    if (!/^\s*@example\b/.test(l)) return [];
    const example = exampleBody(lines.slice(k + 1));
    return example ? [{ ...example, claim }] : [];
  });
}

function exampleBody(following: string[]): Omit<CommentExample, 'claim'> | undefined {
  let body = following.slice(
    0,
    indexOrEnd(following, (l) => /^\s*@\w/.test(l)),
  );
  while (body[0]?.trim() === '') body = body.slice(1);
  if (body[0] !== undefined && /^\s*```/.test(body[0])) {
    body = body.slice(1);
    body = body.slice(
      0,
      indexOrEnd(body, (b) => /^\s*```\s*$/.test(b)),
    );
  }
  const tag = /^\s*\/\/\s*(check|compile)(\s+network)?\s*$/.exec(body[0] ?? '');
  if (!tag) return undefined;
  const mode = tag[1] === 'check' ? 'check' : 'compile';
  return {
    mode,
    network: tag[2] !== undefined,
    code: body.slice(1).join('\n').replace(/\s+$/, ''),
  };
}

function indexOrEnd(lines: string[], test: (line: string) => boolean): number {
  const index = lines.findIndex(test);
  return index < 0 ? lines.length : index;
}

function exportedName(statement: ts.Statement): string | undefined {
  const modifiers = ts.canHaveModifiers(statement) ? (ts.getModifiers(statement) ?? []) : [];
  if (!modifiers.some((m) => m.kind === ts.SyntaxKind.ExportKeyword)) return undefined;
  if (modifiers.some((m) => m.kind === ts.SyntaxKind.DefaultKeyword)) return 'default';
  if (ts.isVariableStatement(statement)) {
    const declaration = statement.declarationList.declarations[0];
    return declaration && ts.isIdentifier(declaration.name) ? declaration.name.text : undefined;
  }
  const name = (statement as ts.Statement & { name?: ts.Node }).name;
  return name && ts.isIdentifier(name) ? name.text : undefined;
}
