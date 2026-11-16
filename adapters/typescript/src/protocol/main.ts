#!/usr/bin/env node
// docattest-adapter-typescript: the TypeScript adapter behind the protocol in PROTOCOL.md.
//
// Attested code may print, on any thread and by any means, down to writing descriptor 1
// directly. So the server runs as a child whose descriptor 1 is this process's stderr, and
// whose descriptor 3 is this process's stdout: only the server's responses reach the core.
// Stdin is handed over as it is; nothing flows through this process.
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { PROTOCOL_FD_FLAG } from './output.js';

const server = fileURLToPath(new URL('./serve.js', import.meta.url));
const args = [...process.execArgv, server, PROTOCOL_FD_FLAG];
const child = spawn(process.execPath, args, { stdio: [0, 2, 2, 1] });
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.on(signal, () => child.kill(signal));
}
child.on('error', (err) => {
  process.stderr.write(`docattest-adapter-typescript: ${err.message}\n`);
  process.exit(1);
});
child.on('exit', (code) => process.exit(code ?? 1));
