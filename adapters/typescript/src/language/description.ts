import type { LanguageDescription } from '../api/adapter.js';
import { BLOCK_LANGUAGES, IGNORED_NAMES, isSourceFile, SOURCE_FILES } from './languages.js';
import { docCommentProse } from './syntax/doc-prose.js';
import { asserts, expressionShape, mentioned } from './syntax/lexical.js';
import { observe } from './worker/observe.js';

/**
 * What TypeScript and JavaScript say of themselves without the compiler: this module and
 * everything it imports load in a few milliseconds, so `hello` is answered at once.
 */
export const description: LanguageDescription = {
  blockLanguages: BLOCK_LANGUAGES,
  sourceFiles: SOURCE_FILES,
  ignoredNames: IGNORED_NAMES,
  vocabulary: {
    untitled: '(computed title)',
    noTests: 'it declares no test with a literal title',
  },
  assertions: (examples) => Promise.resolve(examples.map((e) => asserts(e.code))),
  docProse: (files) =>
    Promise.resolve(files.map((f) => (isSourceFile(f.page) ? docCommentProse(f.text) : []))),
  mentions: (spans, symbols) => {
    const wanted = new Set(symbols);
    return Promise.resolve(spans.map((span) => mentioned(span, wanted)));
  },
  expressions: (expressions) => Promise.resolve(expressions.map(expressionShape)),
  display: (values) => Promise.resolve(values.map((value) => observe(value).text)),
};
