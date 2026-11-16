// A local server for `refresh` (tests/parity/impact.ts): nothing in the impact cases
// touches the network.
import {
  createServer,
  type IncomingMessage,
  type RequestListener,
  type ServerResponse,
} from 'node:http';
import type { AddressInfo } from 'node:net';
import { gzipSync } from 'node:zlib';

/** Paths with a fixed body: its content type and text. */
const BODIES: Readonly<Record<string, readonly [string, string]>> = {
  '/regions': ['application/json', JSON.stringify({ regions: ['eu', 'us'], count: 2 })],
  '/order': [
    'application/json',
    '{"b":1.0,"10":"x","2":[1e21,-0,1.50,{}],"a":{"é":"\\u00e9\\n","😀":null},"deep":[[[]]]}',
  ],
  '/bom': ['application/json', '﻿{"bom":true}'],
  '/scalar': ['text/plain', '42'],
  '/html': ['text/html', '<html>nope</html>'],
  '/empty': ['application/json', ''],
  '/surrogate': [
    'application/json',
    '{"\\ud800":"a\\udfffb\\ud83d\\ude00\\\\ud800","k":"\\uD83Dx"}',
  ],
};
// Paths that answer something other than a fixed body.
type Route = (req: IncomingMessage, res: ServerResponse) => void;
const ROUTES: Readonly<Record<string, Route>> = {
  '/moved': (_req, res) => res.writeHead(302, { location: '/regions' }).end(),
  '/loop': (_req, res) => res.writeHead(302, { location: '/loop' }).end(),
  '/missing': (_req, res) =>
    res.writeHead(404, { 'content-type': 'application/json' }).end('{"error":"gone"}'),
  '/broken': (_req, res) => res.writeHead(500).end('boom'),
  '/nothing': (_req, res) => res.writeHead(204).end(),
  '/gzip': (req, res) => {
    const zipped = (req.headers['accept-encoding'] ?? '').includes('gzip');
    const text = JSON.stringify({ zipped: 'either way', n: [1, 2, 3] });
    res.writeHead(200, {
      'content-type': 'application/json',
      ...(zipped ? { 'content-encoding': 'gzip' } : {}),
    });
    res.end(zipped ? gzipSync(text) : text);
  },
  '/agent': (req, res) => {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(
      JSON.stringify({
        agent: req.headers['user-agent'] ?? null,
        accept: req.headers.accept ?? null,
      }),
    );
  },
};
const answer: RequestListener = (req, res) => {
  const path = new URL(req.url ?? '/', 'http://x').pathname;
  const route = ROUTES[path];
  if (route) {
    route(req, res);
    return;
  }
  const found = BODIES[path];
  if (!found) {
    res.writeHead(404).end('no');
    return;
  }
  res.writeHead(200, { 'content-type': found[0] });
  res.end(found[1]);
};

/** The server, listening on a port of its own on the loopback interface. */
export const server = createServer(answer);
await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));

/** The port a server listens on. */
const portOf = (listening: ReturnType<typeof createServer>): number =>
  (listening.address() as AddressInfo).port;

/** Where the server answers: `http://127.0.0.1:<port>`. */
export const URL_BASE = `http://127.0.0.1:${portOf(server)}`;
// A port nothing listens on: taken, then released.
const closed = createServer();
await new Promise<void>((done) => closed.listen(0, '127.0.0.1', done));
/** A URL nothing answers at. */
export const DEAD = `http://127.0.0.1:${portOf(closed)}`;
await new Promise<void>((done) => {
  closed.close(() => {
    done();
  });
});
