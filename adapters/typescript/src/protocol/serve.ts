// The adapter server proper, started by main.ts with its stdout on the launcher's stderr
// and the launcher's stdout on descriptor 3. Nothing but responses is written there.
import { readFileSync } from 'node:fs';
import { enableCompileCache } from 'node:module';
import { protocolOutput } from './output.js';

// The compiler is megabytes of JavaScript, loaded by every adapter process: Node's own cache
// of compiled code (in the temporary directory, or NODE_COMPILE_CACHE) saves most of that.
enableCompileCache();

const output = protocolOutput(process.argv.slice(2));
const { serve } = await import('./server.js');
// The description loads at once; the compiler only when a request needs it.
const { description } = await import('../language/description.js');
const pkg = JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8')) as {
  version: string;
};

await serve({
  input: process.stdin,
  write: output.line,
  description,
  load: async () => (await import('../language/index.js')).typescript,
  info: { name: 'typescript', version: pkg.version },
});
// Exit once the last line is out: attested code may have left handles behind.
await output.end();
process.exit(0);
