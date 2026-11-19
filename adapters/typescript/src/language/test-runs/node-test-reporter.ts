// A reporter for `node --test`, which Node loads from the adapter's dist by its URL: one JSON
// line for each event the adapter reads, so that the outcome of a test comes from the
// runner's own events and never from its exit code or its printed text.
import type { TestEvent } from 'node:test/reporters';

/** One line the reporter writes; `readNodeTestReport` reads them back. */
export type NodeTestLine =
  | { event: 'start'; name: string; nesting: number }
  | {
      event: 'pass' | 'fail';
      name: string;
      nesting: number;
      suite: boolean;
      skip: string | boolean;
      todo: string | boolean;
      message: string | undefined;
    }
  | { event: 'stderr'; message: string };

/** The reporter: Node passes it the run's events and writes what it yields. */
export default async function* nodeTestReporter(
  source: AsyncIterable<TestEvent>,
): AsyncGenerator<string> {
  for await (const event of source) {
    const line = lineOf(event);
    if (line) yield `${JSON.stringify(line)}\n`;
  }
}

function lineOf(event: TestEvent): NodeTestLine | undefined {
  switch (event.type) {
    case 'test:start':
      return { event: 'start', name: event.data.name, nesting: event.data.nesting };
    case 'test:pass': {
      const { name, nesting, details, skip = false, todo = false } = event.data;
      return {
        event: 'pass',
        name,
        nesting,
        suite: details.type === 'suite',
        skip,
        todo,
        message: undefined,
      };
    }
    case 'test:fail': {
      const { name, nesting, details, skip = false, todo = false } = event.data;
      const message = details.error.message;
      return { event: 'fail', name, nesting, suite: details.type === 'suite', skip, todo, message };
    }
    case 'test:stderr':
      return { event: 'stderr', message: event.data.message };
    default:
      return undefined;
  }
}
