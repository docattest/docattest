import type { Language, LanguageDescription } from '../api/adapter.js';
import { Methods, type AdapterInfo } from './methods.js';
import { idOf, parseRequest, Params, wireError, type RequestId, type Response } from './wire.js';

/** What a server reads, writes and serves. */
interface ServerOptions {
  input: NodeJS.ReadableStream;
  /** Writes one response line. */
  write: (line: string) => void;
  /** What the language says of itself at once: `hello` and the questions about text. */
  description: LanguageDescription;
  /** Loads the whole language, compiler and all; called once, when first needed. */
  load: () => Promise<Language>;
  info: AdapterInfo;
}

/** A request whose handler has been called: its id, and the answer on its way. */
interface Started {
  id: RequestId | null;
  answer: Promise<unknown>;
}

/**
 * Serve the adapter protocol until the input ends. Each request starts as soon as its line
 * arrives and is answered when it finishes, in whatever order that is. At end of input the
 * server waits for requests in flight, closes the session and resolves.
 *
 * `hello` is answered from the description alone. The whole language is loaded when a
 * request needs it, or right after a `hello` that said more will follow (`warm`); requests
 * that arrive meanwhile wait their turn, so they still start in the order they were sent.
 */
export async function serve(options: ServerOptions): Promise<void> {
  const { input, write, description, load, info } = options;
  const methods = new Methods(description, info);
  const inFlight = new Set<Promise<void>>();
  let loading: Promise<void> | undefined;
  const loaded = () =>
    (loading ??= load().then((language) => {
      methods.use(language);
    }));
  const start = inTurn(methods, loaded);
  const accept = (line: string) => {
    if (!line.trim()) return;
    const done = start(line)
      .then(answered)
      .then((response) => {
        write(encode(response));
        // The answer to `hello` is out: now is the time to get ready for what follows it.
        if (methods.warm && !loading) void loaded().catch(() => undefined);
      });
    inFlight.add(done);
    void done.finally(() => inFlight.delete(done));
  };
  await readLines(input, accept);
  while (inFlight.size) await Promise.all(inFlight);
  try {
    await methods.shutdown();
  } catch (err) {
    process.stderr.write(`${wireError(err).message}\n`);
  }
}

/**
 * Start requests one after another, in arrival order: each handler is called once the one
 * before it has been, and once the language is there if it needs it. The answers are not
 * waited for, so requests still overlap.
 */
function inTurn(methods: Methods, loaded: () => Promise<void>): (line: string) => Promise<Started> {
  let turn: Promise<unknown> = Promise.resolve();
  return (line) => {
    const started = turn.then(async (): Promise<Started> => {
      let id: RequestId | null = null;
      try {
        const request = parseRequest(line);
        id = request.id;
        if (methods.needs(request.method)) await loaded();
        const params = new Params(request.method, request.params);
        return { id, answer: Promise.resolve(methods.call(request.method, params)) };
      } catch (err) {
        return { id: id ?? idOf(err), answer: Promise.reject(toError(err)) };
      }
    });
    turn = started;
    return started;
  };
}

async function answered({ id, answer }: Started): Promise<Response> {
  try {
    return { id, result: await answer };
  } catch (err) {
    return { id, error: wireError(err) };
  }
}

const toError = (err: unknown) => (err instanceof Error ? err : new Error(String(err)));

// A result that cannot be serialised is the adapter's bug, reported against its request.
function encode(response: Response): string {
  try {
    return JSON.stringify(response);
  } catch (err) {
    return JSON.stringify({ id: response.id, error: wireError(err) });
  }
}

/** Call `accept` with every newline-terminated line, and with a final unterminated one. */
async function readLines(input: NodeJS.ReadableStream, accept: (line: string) => void) {
  input.setEncoding('utf8');
  // Only new text is scanned, so a long line arriving in many chunks costs linear time.
  let parts: string[] = [];
  for await (const chunk of input) {
    const text = String(chunk);
    let start = 0;
    for (let end = text.indexOf('\n'); end !== -1; end = text.indexOf('\n', start)) {
      accept(parts.join('') + text.slice(start, end));
      parts = [];
      start = end + 1;
    }
    if (start < text.length) parts.push(text.slice(start));
  }
  if (parts.length) accept(parts.join(''));
}
