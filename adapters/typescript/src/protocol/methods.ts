import type {
  AdapterSession,
  CitedTest,
  Language,
  LanguageDescription,
  SourceText,
} from '../api/adapter.js';
import type { Attestation } from '../api/attestation.js';
import { InternalError, UserError } from '../api/errors.js';
import { sourceFileMatcher } from '../files/glob.js';
import { ignoreRule } from '../files/ignored.js';
import type { FileSource } from '../files/source.js';
import { isObject, isStringArray } from '../json.js';
import { changeOf, contextOf } from './context.js';
import { Params, PROTOCOL_VERSION, ProtocolError } from './wire.js';

/** Who this adapter is, for `hello`. */
export interface AdapterInfo {
  name: string;
  version: string;
}

/** The optional protocol methods (PROTOCOL.md §3): this adapter implements every one. */
const CAPABILITIES: readonly string[] = [
  'tests',
  'runTests',
  'observables',
  'affected',
  'display',
  'mutants',
];

type Handler = (params: Params) => unknown;
interface Opened {
  session: AdapterSession;
  files: FileSource;
}
type SessionHandler = (opened: Opened, params: Params) => unknown;

/** The methods that need the whole language: its compiler, its runtime. */
const HEAVY: readonly string[] = ['docComments', 'open'];

/**
 * The protocol's methods over one language plugin and the one session `open` creates. Each
 * handler runs synchronously up to its first await, in the order requests arrive, so `open`
 * has taken effect before the next line is read.
 *
 * It starts with the language's description only, which is enough for `hello` and for the
 * questions about text; the server hands it the whole language (`use`) before it calls a
 * method that `needs` it.
 */
export class Methods {
  private greeted = false;
  private language: Language | undefined;
  private opened: Opened | undefined;
  private closing: Promise<void> | undefined;
  private readonly inFlight = new Set<Promise<unknown>>();
  private readonly isSource: (file: string) => boolean;
  /** Whether the last `hello` said that more requests will follow. */
  warm = false;

  constructor(
    private readonly description: LanguageDescription,
    private readonly info: AdapterInfo,
  ) {
    this.isSource = sourceFileMatcher(description.sourceFiles);
  }

  /** Whether `method` cannot be called before the whole language is handed over. */
  needs(method: string): boolean {
    return this.greeted && !this.language && HEAVY.includes(method);
  }

  /** Hand over the whole language. */
  use(language: Language): void {
    this.language = language;
  }

  /** Call one method; throws for an unknown method or one out of order. */
  call(method: string, params: Params): unknown {
    const { description } = this;
    const lifecycle = new Map<string, Handler>([
      ['hello', (p) => this.hello(p)],
      ['docComments', (p) => this.docComments(p)],
      ['assertions', (p) => description.assertions(examplesOf(p))],
      ['docProse', (p) => description.docProse(sourcesOf('docProse', p))],
      ['mentions', (p) => description.mentions(p.strings('spans'), p.strings('symbols'))],
      ['expressions', (p) => description.expressions(p.strings('expressions'))],
      ['display', (p) => description.display(p.list('values'))],
      ['open', (p) => this.open(p)],
      ['close', () => this.close()],
    ]);
    const handler = lifecycle.get(method) ?? this.sessionMethod(method);
    if (!handler) throw new ProtocolError(`unknown method "${method}"`);
    if (!this.greeted && method !== 'hello') {
      throw new ProtocolError(`"${method}" before "hello": hello is always first`);
    }
    return handler(params);
  }

  /** Close the session if one is open and not yet closed. */
  async shutdown(): Promise<void> {
    if (this.opened && !this.closing) this.closing = this.opened.session.close();
    await this.closing;
  }

  /**
   * `close`: release the session's workers once every request already sent has its answer,
   * so a `close` that overtakes them never cuts them short. Later session methods are errors.
   */
  private async close(): Promise<unknown> {
    const { session } = this.current('close');
    this.closing = Promise.allSettled([...this.inFlight]).then(() => session.close());
    await this.closing;
    return {};
  }

  private hello(params: Params): unknown {
    const protocol = params.number('protocol');
    if (protocol !== PROTOCOL_VERSION) {
      throw new UserError(
        `the ${this.info.name} adapter speaks protocol ${PROTOCOL_VERSION}, not ${protocol}`,
      );
    }
    this.greeted = true;
    this.warm = params.has('warm') && params.boolean('warm');
    return {
      protocol: PROTOCOL_VERSION,
      adapter: { name: this.info.name, version: this.info.version },
      blockLanguages: [...this.description.blockLanguages],
      sourceFiles: {
        include: [...this.description.sourceFiles.include],
        exclude: [...this.description.sourceFiles.exclude],
      },
      ignoredNames: [...this.description.ignoredNames],
      capabilities: [...CAPABILITIES],
      vocabulary: { ...this.description.vocabulary },
    };
  }

  private whole(method: string): Language {
    if (this.language) return this.language;
    throw new InternalError(`"${method}" was called before the language loaded`);
  }

  private docComments(params: Params): unknown {
    const language = this.whole('docComments');
    return sourcesOf('docComments', params).flatMap(({ page, text }) =>
      this.isSource(page) ? language.docComments(page, text) : [],
    );
  }

  private open(params: Params): unknown {
    if (this.opened) throw new ProtocolError('"open" was already called: one session per process');
    const context = contextOf(params, ignoreRule(this.description));
    this.opened = { session: this.whole('open').createSession(context), files: context.files };
    return {};
  }

  private sessionMethod(method: string): Handler | undefined {
    const handler = SESSION_METHODS.get(method);
    if (!handler) return undefined;
    return (params) => {
      const answer = Promise.resolve(handler(this.current(method), params));
      this.inFlight.add(answer);
      void answer.finally(() => this.inFlight.delete(answer)).catch(() => undefined);
      return answer;
    };
  }

  private current(method: string): Opened {
    if (!this.opened) throw new ProtocolError(`"${method}" before "open"`);
    if (this.closing) throw new ProtocolError(`"${method}" after "close"`);
    return this.opened;
  }
}

const SESSION_METHODS = new Map<string, SessionHandler>([
  ['compile', ({ session }, p) => session.compile(p.attestations('examples', 'example'))],
  [
    'evaluate',
    async ({ session }, p) =>
      Object.fromEntries(await session.evaluate(p.attestations('facts', 'fact'))),
  ],
  [
    'run',
    ({ session }, p) =>
      session.run(p.attestations('examples', 'example'), p.attestations('cases', 'cases')),
  ],
  ['symbols', ({ session }, p) => symbolsById(session, p.attestations('attestations'))],
  ['exports', async ({ session }, p) => (await session.exports(p.string('surface'))) ?? null],
  ['describeSurfaces', async ({ session }) => Object.fromEntries(await session.describeSurfaces())],
  ['tests', ({ session }, p) => session.tests(p.string('file'))],
  ['runTests', ({ session }, p) => session.runTests(citedTestsOf(p), p.number('timeoutMs'))],
  ['observables', ({ session }, p) => session.observables(p.string('surface'))],
  ['affected', ({ session, files }, p) => session.affected(changeOf(files, p))],
  ['inCode', ({ session }, p) => session.inCode(p.strings('names'), p.strings('files'))],
  ['mutants', ({ session }, p) => session.mutants(p.symbolRefs('symbols'), p.number('limit'))],
]);

/** `files`: source files as `{ page, text }`. */
function sourcesOf(method: string, params: Params): SourceText[] {
  return params.list('files').map((file) => {
    const { page, text } = isObject(file) ? file : {};
    if (typeof page !== 'string' || typeof text !== 'string') {
      throw new ProtocolError(`${method}: each of "files" must have a "page" and a "text"`);
    }
    return { page, text };
  });
}

/** `examples`: example code as `{ lang, code }`. */
function examplesOf(params: Params): { lang: string; code: string }[] {
  return params.list('examples').map((example) => {
    const { lang, code } = isObject(example) ? example : {};
    if (typeof lang !== 'string' || typeof code !== 'string') {
      throw new ProtocolError('assertions: each of "examples" must have a "lang" and a "code"');
    }
    return { lang, code };
  });
}

/** `tests`: the tests citations name, as `{ file, describes, title }`. */
function citedTestsOf(params: Params): CitedTest[] {
  return params.list('tests').map((test) => {
    const { file, describes, title } = isObject(test) ? test : {};
    if (typeof file !== 'string' || !isStringArray(describes) || typeof title !== 'string') {
      throw new ProtocolError(
        'runTests: each of "tests" must have a "file", "describes" and a "title"',
      );
    }
    return { file, describes, title };
  });
}

async function symbolsById(session: AdapterSession, all: readonly Attestation[]) {
  const refs = await Promise.all(all.map(async (a) => [a.id, await session.symbols(a)] as const));
  return Object.fromEntries(refs);
}
