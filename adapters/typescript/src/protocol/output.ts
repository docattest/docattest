import { createWriteStream, fstatSync } from 'node:fs';
import { Socket } from 'node:net';
import { isatty, WriteStream } from 'node:tty';

/** The descriptor the server writes responses to; the launcher maps it to its own stdout. */
const PROTOCOL_FD = 3;

/** The argument by which the launcher tells the server that descriptor is its to write. */
export const PROTOCOL_FD_FLAG = `--protocol-fd=${PROTOCOL_FD}`;

/** The protocol's own way out. */
interface ProtocolOutput {
  /** Write one line. */
  readonly line: (text: string) => void;
  /** Resolves once every line written has been handed to the operating system. */
  readonly end: () => Promise<void>;
}

/**
 * Responses go to a descriptor nothing else in the process knows about. The server's own
 * stdout (descriptor 1, which attested code on any thread can reach with
 * `fs.writeSync(1, …)`) is the launcher's stderr, so no output but a response can ever
 * reach the core's end of the pipe.
 */
export function protocolOutput(argv: readonly string[]): ProtocolOutput {
  // Without the launcher, descriptor 3 is whatever Node itself opened first.
  if (!argv.includes(PROTOCOL_FD_FLAG)) {
    throw new Error(
      'the adapter server writes responses to a descriptor its launcher opens: ' +
        'start it with docattest-adapter-typescript (dist/protocol/main.js)',
    );
  }
  const stream = writable(PROTOCOL_FD);
  return {
    line: (text) => {
      stream.write(`${text}\n`);
    },
    end: () =>
      new Promise((resolve) => {
        stream.end(resolve);
      }),
  };
}

// The same choice Node makes for process.stdout: a terminal, a pipe or socket, or anything
// else (a file, or a device such as /dev/null), which is written like a file.
function writable(fd: number): NodeJS.WritableStream {
  if (isatty(fd)) return new WriteStream(fd);
  const stat = fstatSync(fd);
  if (stat.isFIFO() || stat.isSocket()) return new Socket({ fd, readable: false, writable: true });
  return createWriteStream('', { fd });
}
