import * as net from 'node:net';

const NETWORK_OFF =
  'network access is disabled for attestations; tag the block `check network` to allow it';

const refuse = (): never => {
  throw new Error(NETWORK_OFF);
};

// What the guard replaced, restored when the outermost guarded call ends.
const saved: { depth: number; fetch: typeof fetch; connect: PropertyDescriptor | undefined } = {
  depth: 0,
  fetch: globalThis.fetch,
  connect: undefined,
};

/** Run with `fetch` and socket connections refused. Nested calls share one guard. */
export async function offline<T>(run: () => T | Promise<T>): Promise<T> {
  if (saved.depth++ === 0) {
    saved.fetch = globalThis.fetch;
    saved.connect = Object.getOwnPropertyDescriptor(net.Socket.prototype, 'connect');
    globalThis.fetch = () => Promise.reject(new Error(NETWORK_OFF));
    Object.defineProperty(net.Socket.prototype, 'connect', {
      value: refuse,
      configurable: true,
      writable: true,
    });
  }
  try {
    return await run();
  } finally {
    if (--saved.depth === 0) {
      globalThis.fetch = saved.fetch;
      if (saved.connect) Object.defineProperty(net.Socket.prototype, 'connect', saved.connect);
    }
  }
}
