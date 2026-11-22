import type { FileSource } from '../files/source.js';
import type {
  Attestation,
  CasesAttestation,
  ExampleAttestation,
  FactAttestation,
} from './attestation.js';
import type { SurfaceEntry } from './config.js';
import type { Result, SurfaceResolution } from './report.js';

/** A compiler message for one example; `line` is within the block. */
export interface Diagnostic {
  id: string;
  message: string;
  line?: number;
}

/** A fact's value as observed by the runtime: what the comparison needs, as plain data. */
export interface Observation {
  /** The value as the language itself displays it: `String(value)` here, `str(value)` in Python. */
  text: string;
  /** Each item as a string, when the value is a list, a set or another iterable. */
  items?: string[];
  /** A short description for messages, such as `an object` or `number 3`. */
  description: string;
  /**
   * `text` when the value is text, a string of the language: the one thing about a value's
   * type the core needs (only text can be what a fenced block copies). Absent otherwise.
   */
  kind?: 'text';
}

/** The outcome of evaluating one fact expression. */
export type Evaluation = { ok: true; value: Observation } | { ok: false; message: string };

/** A surface symbol an attestation exercises. `surface` is absent when none exports it. */
export interface SymbolRef {
  name: string;
  surface?: string;
}

/** A test or describe block a test file declares, by its literal title. */
export interface DeclaredTest {
  title: string;
  /** Enclosing describe titles, outermost first; null where a title is not a literal. */
  describes: (string | null)[];
  line: number;
}

/**
 * Something a test file declares that can be cited by mistake for a test: a describe
 * block. `kind` says what it is, in the language's words, with its article: the core's
 * message reads `"x" in a.test.ts is <kind>, not a test`.
 */
export interface DeclaredSuite extends DeclaredTest {
  kind: string;
}

/** What a test file declares, or why it cannot be read. */
export type TestListing =
  { ok: true; tests: DeclaredTest[]; suites: DeclaredSuite[] } | { ok: false; message: string };

/** A test a citation names, as its file declares it: the `describes` and `title` it lists. */
export interface CitedTest {
  file: string;
  describes: string[];
  title: string;
}

/**
 * How a cited test ran: it passed; or it failed, was skipped, or nothing ran (`notRun`),
 * with what the runner said about it, a few lines.
 */
export type TestRun =
  { status: 'passed' } | { status: 'failed' | 'skipped' | 'notRun'; message: string };

/**
 * The language's words for what the core's sentences name. Each is optional; the core has
 * a default that says the same thing in no language's terms.
 */
export interface Vocabulary {
  /** Stands for an enclosing title that cannot be cited, in a test's path. Default `(untitled)`. */
  untitled?: string;
  /** What a file with no test to cite is said to do. Default `it declares no test`. */
  noTests?: string;
}

/** What the protocol server asks of the language for one project (PROTOCOL.md §3). */
export interface Adapter {
  /** Type-check examples; diagnostics are keyed by attestation id. */
  compile(examples: readonly ExampleAttestation[]): Promise<Diagnostic[]>;
  /** Evaluate fact expressions against their surfaces. Pure: no side effects. */
  evaluate(facts: readonly FactAttestation[]): Promise<Map<string, Evaluation>>;
  /** Run examples and cases tables in isolation. */
  run(
    examples: readonly ExampleAttestation[],
    cases: readonly CasesAttestation[],
  ): Promise<Result[]>;
  /** Surface symbols an attestation exercises. */
  symbols(attestation: Attestation): Promise<SymbolRef[]>;
  /** Public names a surface offers, or undefined when it cannot be resolved. */
  exports(surface: string): Promise<string[] | undefined>;
  /** The tests and describe blocks a project-relative test file declares, from its syntax. */
  tests(file: string): Promise<TestListing>;
  /**
   * Run each cited test with the project's own runner, exactly that test selected, and say
   * how it ran, in order. `timeoutMs` bounds one run of the runner.
   */
  runTests(tests: readonly CitedTest[], timeoutMs: number): Promise<TestRun[]>;
}

/**
 * A surface as the config states it. What the entry means (a file of the project, a package,
 * a module, a crate) is the adapter's to say; the core only knows its name.
 */
export interface ConfiguredSurface {
  name: string;
  /** The config's value for this surface, as written. */
  entry: SurfaceEntry;
}

/** How one surface resolved, for the report. */
export interface SurfaceDescription {
  resolution?: SurfaceResolution;
  warnings: string[];
}

/** What an adapter is built from: one project, for one check or query. */
export interface AdapterContext {
  files: FileSource;
  /** The config's surfaces, in its order, as written. */
  surfaces: readonly ConfiguredSurface[];
  setup: string | undefined;
  timeoutMs: number;
  workers: number;
  /** Whether `check network` examples may run. */
  network: boolean;
  snapshots: { dir: string | undefined; data: ReadonlyMap<string, unknown> };
  /** The data of each manifest the facts name, by the path as they write it. */
  manifests: ReadonlyMap<string, unknown>;
  /** The text of each file the facts read with `file`, by the path as they write it. */
  texts: ReadonlyMap<string, string>;
  /** What each regex facts give `matched` matches, by the path, then the regex, as written. */
  matches: ReadonlyMap<string, Readonly<Record<string, string>>>;
  /** The value of each field facts read with `manifestField`, by the path, then the keys. */
  manifestFields: ReadonlyMap<string, Readonly<Record<string, unknown>>>;
  /** The examples this session will compile; the program is built around them. */
  examples: readonly ExampleAttestation[];
}

/** A value a surface offers a reader, with the fact expression that reads it. */
export interface Observable {
  surface: string;
  /** A fact expression that evaluates to `value`. */
  expression: string;
  /** Names on the way to the value: the export, then the members, then a field. */
  path: string[];
  value: Observation;
}

/** The project at a base revision, for `impact`. */
export interface SourceChange {
  /** The project's files as they were at the base: same root, base contents. */
  base: FileSource;
  /** Project-relative paths whose contents differ between the base and now. */
  changed: readonly string[];
}

/** An adapter for one check, plus what the report and the lifecycle need. */
export interface AdapterSession extends Adapter {
  /** How each configured surface resolved, by surface name. */
  describeSurfaces(): Promise<Map<string, SurfaceDescription>>;
  /**
   * Values a surface exports, nested ones included, each with the fact expression that
   * reads it, for `audit`. Throws a `UserError` when the surface cannot be loaded.
   */
  observables(surface: string): Promise<Observable[]>;
  /**
   * Surface exports whose declaration changed since the base, or that reach a changed
   * declaration through the code they use.
   */
  affected(change: SourceChange): Promise<SymbolRef[]>;
  /**
   * Which of `names` still occur as a name in the project's code: in the project-relative
   * `files` (the ones the config's `sources` names) and in the files the surfaces are read
   * from. An adapter that cannot tell returns every name.
   */
  inCode(names: readonly string[], files: readonly string[]): Promise<string[]>;
  /** Release workers and anything else the session holds. */
  close(): Promise<void>;
}

/**
 * Which project files are source files of a language, as data the core applies without
 * knowing the language: a file is one when its project-relative path matches a glob of
 * `include` and no glob of `exclude`. The globs are read like the config's own
 * (`sourceFileMatcher` in `src/files/glob.ts`): whole path, dotfiles match, case matters.
 */
export interface SourceFiles {
  readonly include: readonly string[];
  readonly exclude: readonly string[];
}

/** A source file, or what stands for one: its project-relative path and its text. */
export interface SourceText {
  page: string;
  text: string;
}

/** The prose of one doc comment: its description as a claim is written, and its lines. */
export interface DocProse {
  /** The description on one line: comments removed, whitespace collapsed, markup kept. */
  text: string;
  /** The comment's first and last line, counted from 1. */
  start: number;
  end: number;
}

/** A fact expression with its names taken out. */
export interface ExpressionShape {
  /**
   * The expression with every name replaced by a placeholder. Two expressions with the same
   * shape differ in nothing but their names; shapes are only ever compared for equality.
   */
  shape: string;
  /** The names, in the order they occur; a name used twice is listed twice. */
  names: string[];
}

/**
 * What a language says of itself without its compiler or runtime, so that it is answered
 * at once: static data, and questions about text that need no project. Every method takes
 * many items and answers one for each, in order.
 */
export interface LanguageDescription {
  /** Info-string languages whose fenced blocks this language runs, lower case. */
  readonly blockLanguages: readonly string[];
  /** The project files that are this language's source code: doc comments may hold examples. */
  readonly sourceFiles: SourceFiles;
  /**
   * Names of files and directories that are never the project's own, wherever they are:
   * installed dependencies, build output. Each is a glob for one name (`*.egg-info`), never
   * for a path. A path with such a segment is not a project file.
   */
  readonly ignoredNames: readonly string[];
  /** The language's words for what the core's sentences name. */
  readonly vocabulary: Vocabulary;
  /** Whether each example's code asserts anything; one that does not is weak coverage. */
  assertions(examples: readonly { lang: string; code: string }[]): Promise<boolean[]>;
  /** The doc comments of each source file as prose, in the order they are written. */
  docProse(files: readonly SourceText[]): Promise<DocProse[][]>;
  /**
   * For each inline code span of a page, the one of `symbols` it names, or null. A name is
   * always part of the span that names it.
   */
  mentions(spans: readonly string[], symbols: readonly string[]): Promise<(string | null)[]>;
  /** The shape and the names of each fact expression. */
  expressions(expressions: readonly string[]): Promise<ExpressionShape[]>;
  /**
   * Each value of a data block, as JSON holds it, displayed as a fact's value of the same value
   * is: what a `field=` fact compares.
   */
  display(values: readonly unknown[]): Promise<string[]>;
}

/** The whole language: its description, how to find its examples, and its sessions. */
export interface Language extends LanguageDescription {
  /** `@example` attestations in one source file's doc comments. */
  docComments(page: string, text: string): ExampleAttestation[];
  /** Build the adapter for one check or query. */
  createSession(context: AdapterContext): AdapterSession;
}
