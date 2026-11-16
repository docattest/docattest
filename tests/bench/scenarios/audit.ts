// Benchmark scenarios: audit. ../scenarios.ts joins the parts, in order.
import { lines } from '../../lib/lines.ts';
import { CONFIG, lineOf, project } from '../fixture.ts';
import { suggests, suggestsNothing } from '../judges.ts';
import { adder, type Scenario } from '../scenario.ts';

export const scenarios: Scenario[] = [];
const add = adder(scenarios);

// =====================================================================================
// audit: find claims nobody attested. Pages here carry no directives at all.
// =====================================================================================
export const AUDIT = ['audit', '--json'];

{
  // CORPUS #1: an env var list that fell behind the code.
  const page = lines(
    '# Service',
    '',
    'Environment variables override the file:',
    '',
    '- `API_TOKEN`',
    '- `REGION`',
    '- `PORT`',
  );
  add('audit', 'positive', 'audit-01', 'corpus #1: env var list missing three keys', {
    files: project({
      'src/index.ts': `export const ENV_KEYS = ['API_TOKEN', 'REGION', 'PORT', 'TTS_ENDPOINT', 'STT_ENDPOINT', 'RATE'];\n`,
      'README.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'README.md',
        line: lineOf(page, '- `API_TOKEN`'),
        target: 'list',
        status: 'drift',
        mentions: 'ENV_KEYS',
        missing: ['TTS_ENDPOINT', 'STT_ENDPOINT', 'RATE'],
      }),
  });
}

{
  // CORPUS #2: a route table that fell behind the registry.
  const page = lines(
    '# API',
    '',
    '| Endpoint | Purpose |',
    '| --- | --- |',
    '| `GET /api/status` | health |',
    '| `GET /api/horizon` | snapshot |',
    '| `POST /api/say` | speak |',
    '| `GET /api/voices` | list voices |',
  );
  add('audit', 'positive', 'audit-02', 'corpus #2: endpoint table missing two routes', {
    files: project({
      'src/index.ts': `export const ROUTES = ['GET /api/status', 'GET /api/horizon', 'POST /api/say', 'GET /api/voices', 'GET /api/distractors', 'POST /api/sync'];\n`,
      'README.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'README.md',
        line: lineOf(page, '| Endpoint |'),
        target: 'column',
        status: 'drift',
        mentions: 'ROUTES',
        missing: ['GET /api/distractors', 'POST /api/sync'],
      }),
  });
}

{
  // CORPUS #4: hand-copied error codes, one short.
  const page = lines(
    '# Errors',
    '',
    '`error.code` is one of:',
    '',
    '- `RPC_ERROR`',
    '- `NO_SESSION`',
    '- `TIMEOUT`',
  );
  add(
    'audit',
    'positive',
    'audit-03',
    'corpus #4: error code list missing a key of a const object',
    {
      files: project({
        'src/index.ts': `export const ErrorCodes = { RPC_ERROR: 'rpc', NO_SESSION: 'session', TIMEOUT: 'timeout', AUTH_ERROR: 'auth' } as const;\n`,
        'docs/errors.md': page,
      }),
      run: AUDIT,
      expect: (out) =>
        suggests(out, {
          page: 'docs/errors.md',
          line: lineOf(page, '- `RPC_ERROR`'),
          target: 'list',
          status: 'drift',
          mentions: 'ErrorCodes',
          missing: ['AUTH_ERROR'],
        }),
    },
  );
}

{
  // CORPUS #5: "supports two networks" when the enum has three.
  const page = lines(
    '# Configuration',
    '',
    'The SDK supports these networks:',
    '',
    '- `production`',
    '- `staging`',
  );
  add(
    'audit',
    'positive',
    'audit-04',
    'corpus #5: network list missing a value of a const object',
    {
      files: project({
        'src/index.ts': `export const Network = { PRODUCTION: 'production', STAGING: 'staging', LOCAL: 'local' } as const;\n`,
        'docs/config.md': page,
      }),
      run: AUDIT,
      expect: (out) =>
        suggests(out, {
          page: 'docs/config.md',
          line: lineOf(page, '- `production`'),
          target: 'list',
          status: 'drift',
          mentions: 'Network',
          missing: ['local'],
        }),
    },
  );
}

{
  const page = lines('# Regions', '', 'Jobs can run in:', '', '- `eu`', '- `us`', '- `ap`');
  add('audit', 'positive', 'audit-05', 'a list that matches exactly is a suggested fact', {
    files: project({
      'src/index.ts': `export const REGIONS = ['eu', 'us', 'ap'];\n`,
      'docs/regions.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'docs/regions.md',
        line: lineOf(page, '- `eu`'),
        target: 'list',
        status: 'match',
        mentions: 'REGIONS',
      }),
  });
}

{
  const page = lines(
    '# Routes',
    '',
    '| Method | Path | Purpose |',
    '| --- | --- | --- |',
    '| GET | `/jobs` | list jobs |',
    '| POST | `/jobs` | create a job |',
    '| GET | `/projects` | list projects |',
  );
  add('audit', 'positive', 'audit-06', 'a table column that is a field of an array of objects', {
    files: project({
      'src/index.ts': `export const ROUTES = [{ method: 'GET', path: '/jobs' }, { method: 'POST', path: '/jobs' }, { method: 'GET', path: '/projects' }, { method: 'DELETE', path: '/jobs/:id' }];\n`,
      'docs/routes.md': page,
    }),
    run: AUDIT,
    // The Path column holds `/jobs` twice; items are compared as a set.
    expect: (out) =>
      (out.json?.suggestions ?? []).some(
        (s) =>
          s.page === 'docs/routes.md' &&
          s.line === lineOf(page, '| Method |') &&
          s.target === 'column' &&
          s.status === 'drift' &&
          s.expression.includes('ROUTES') &&
          (s.missing ?? []).includes('/jobs/:id'),
      ),
  });
}

{
  const page = lines('# Queues', '', 'Jobs go to `batch-large` unless told otherwise.');
  add(
    'left-to-binding',
    'positive',
    'audit-07',
    'a distinctive literal that equals an exported value',
    {
      files: project({
        'src/index.ts': `export const DEFAULT_QUEUE = 'batch-large';\nexport const OTHER = 'batch-small';\n`,
        'docs/queues.md': page,
      }),
      run: AUDIT,
      expect: (out) =>
        suggests(out, {
          page: 'docs/queues.md',
          line: lineOf(page, 'batch-large'),
          target: 'scalar',
          status: 'match',
          mentions: 'DEFAULT_QUEUE',
        }),
    },
  );
}

{
  const page = lines(
    '# Scopes',
    '',
    'A token may carry:',
    '',
    '- `read`',
    '- `write`',
    '- `admin`',
    '- `billing`',
  );
  add('audit', 'positive', 'audit-08', 'a list item the code no longer has', {
    files: project({
      'src/index.ts': `export const SCOPES = ['read', 'write', 'admin'];\n`,
      'docs/scopes.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'docs/scopes.md',
        line: lineOf(page, '- `read`'),
        target: 'list',
        status: 'drift',
        mentions: 'SCOPES',
        unexpected: ['billing'],
      }),
  });
}

add('audit', 'negative', 'audit-n1', 'a list of tools that has nothing to do with the surface', {
  files: project({
    'src/index.ts': `export const REGIONS = ['eu', 'us', 'ap'];\n`,
    'docs/setup.md': lines('# Setup', '', 'You need:', '', '- `node`', '- `npm`', '- `git`'),
  }),
  run: AUDIT,
  expect: (out) => suggestsNothing(out),
});

add('audit', 'negative', 'audit-n2', 'a small number that happens to equal an unrelated export', {
  files: project({
    'src/index.ts': `export const MAX_SHARDS = 3;\n`,
    'docs/usage.md': lines('# Usage', '', 'Wait `3` seconds before reading the log.'),
  }),
  run: AUDIT,
  expect: (out) => suggestsNothing(out),
});

add('audit', 'negative', 'audit-n3', 'a list that already carries a fact', {
  files: project({
    'src/index.ts': `export const REGIONS = ['eu', 'us', 'ap'];\n`,
    'docs/regions.md': lines(
      '# Regions',
      '',
      'Jobs can run in: <!-- fact: REGIONS -->',
      '- `eu`',
      '- `us`',
      '- `ap`',
    ),
  }),
  run: AUDIT,
  expect: (out) => suggestsNothing(out),
});

add('audit', 'negative', 'audit-n4', 'a list sharing one item of four with an export', {
  files: project({
    'src/index.ts': `export const COLUMNS = ['id', 'createdAt', 'updatedAt', 'owner'];\n`,
    'docs/form.md': lines(
      '# Form',
      '',
      'The form asks for:',
      '',
      '- `id`',
      '- `name`',
      '- `email`',
      '- `age`',
    ),
  }),
  run: AUDIT,
  expect: (out) => suggestsNothing(out),
});

add('audit', 'negative', 'audit-n5', 'a matching list on a frozen page', {
  files: project(
    {
      'src/index.ts': `export const REGIONS = ['eu', 'us', 'ap'];\n`,
      'docs/old/regions.md': lines('# Regions', '', '- `eu`', '- `us`'),
    },
    { ...CONFIG, frozen: ['docs/old/**'] },
  ),
  run: AUDIT,
  expect: (out) => suggestsNothing(out),
});
