import { receiveMessageOnPort, type MessagePort } from 'node:worker_threads';
import { InternalError } from '../../api/errors.js';
import {
  isHostReply,
  type HostReply,
  type HostRequest,
  type ModuleSource,
  type Resolution,
} from '../run/protocol.js';

/**
 * Asks the session about modules, synchronously: `require` is synchronous, and the session
 * holds the files and the compiler. Answers never change during a check, so they are cached.
 */
export class HostClient {
  private readonly resolutions = new Map<string, Resolution>();
  private readonly sources = new Map<string, ModuleSource | undefined>();

  constructor(
    private readonly port: MessagePort,
    private readonly signal: Int32Array,
  ) {}

  /** How a specifier imported from `from` resolves. */
  resolve(specifier: string, from: string): Resolution {
    const key = `${from}\0${specifier}`;
    let resolution = this.resolutions.get(key);
    if (!resolution) {
      const reply = this.request({ op: 'resolve', specifier, from });
      if (reply.op !== 'resolve') {
        throw new InternalError('the session answered a resolve with something else');
      }
      resolution = reply.resolution;
      this.resolutions.set(key, resolution);
    }
    return resolution;
  }

  /** A project module's code, or undefined when the file does not exist. */
  source(file: string): ModuleSource | undefined {
    if (!this.sources.has(file)) {
      const reply = this.request({ op: 'source', file });
      if (reply.op !== 'source') {
        throw new InternalError('the session answered a source request with something else');
      }
      this.sources.set(file, reply.source);
    }
    return this.sources.get(file);
  }

  // Post, then block until the session flips the signal; its reply is then on the port.
  private request(request: HostRequest): Exclude<HostReply, { op: 'error' }> {
    Atomics.store(this.signal, 0, 0);
    this.port.postMessage(request);
    Atomics.wait(this.signal, 0, 0);
    const reply: unknown = receiveMessageOnPort(this.port)?.message;
    if (!isHostReply(reply)) throw new InternalError('the session did not answer a module request');
    if (reply.op === 'error') throw new Error(reply.message);
    return reply;
  }
}
