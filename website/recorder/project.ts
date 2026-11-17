/** The small invented project every recorded run starts from, as path to text. */
export const PROJECT_FILES: Readonly<Record<string, string>> = {
  'package.json': `${JSON.stringify({ name: 'acme-client', version: '1.0.0', type: 'module' }, null, 2)}\n`,

  'docattest.config.json': `${JSON.stringify(
    {
      pages: ['docs/**/*.md'],
      surfaces: { default: './src/index.ts' },
      setup: 'docs/docattest.setup.ts',
      adapter: 'typescript',
    },
    null,
    2,
  )}\n`,

  'src/index.ts': `export const RETRY = { maxAttempts: 3, backoffMs: 200 };

export const ENV_KEYS = ['API_TOKEN', 'REGION'];

export const ERROR_CODES = ['TIMEOUT', 'RATE_LIMITED', 'NO_SESSION'];

export const backoff = (attempt: number) => RETRY.backoffMs * 2 ** (attempt - 1);

export async function retry<T>(call: () => Promise<T>): Promise<T> {
  let lastError: unknown;
  for (let attempt = 1; attempt <= RETRY.maxAttempts; attempt++) {
    try {
      return await call();
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError;
}
`,

  'test/retry.test.ts': `import { describe, expect, it } from 'vitest';
import { retry } from '../src/index.ts';

describe('retry', () => {
  it('throws the last error', async () => {
    const failing = async () => {
      throw new Error('still down');
    };
    await expect(retry(failing)).rejects.toThrow('still down');
  });
});
`,

  'docs/docattest.setup.ts': `import assert from 'node:assert/strict';
import { backoff } from 'acme-client';

export const cases = {
  backoff: Object.assign(
    (row: Record<string, string>) => {
      assert.equal(String(backoff(Number(row.attempt))), row.result);
    },
    { columns: ['attempt', 'result'] },
  ),
};
`,

  'docs/retries.md': `# Retries

A call that works returns its value unchanged.

\`\`\`ts check
import { retry } from 'acme-client';
assert.equal(await retry(async () => 'ok'), 'ok');
\`\`\`

Tried at most \`3\` times. <!-- fact: RETRY.maxAttempts -->

The client reads these variables:

<!-- fact: ENV_KEYS -->
- \`API_TOKEN\`
- \`REGION\`

The wait doubles each time.

<!-- cases: backoff -->
| case       | attempt | result |
|------------|---------|--------|
| first try  | 1       | 200    |
| second try | 2       | 400    |
| third try  | 3       | 800    |

A call that keeps failing throws the last error. <!-- test: test/retry.test.ts > "throws the last error" -->

Error codes:

- \`TIMEOUT\`
- \`RATE_LIMITED\`
`,
};
