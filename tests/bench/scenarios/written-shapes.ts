// Benchmark scenarios: audit, written shapes. ../scenarios.ts joins the parts, in order.
import { lines } from '../../lib/lines.ts';
import { lineOf, project } from '../fixture.ts';
import { suggests, suggestsNothing } from '../judges.ts';
import { adder, type Outcome, type Scenario } from '../scenario.ts';
import { AUDIT } from './audit.ts';

export const scenarios: Scenario[] = [];
const add = adder(scenarios);

// =====================================================================================
// audit, written shapes: the ways documentation sites tend to write their copied facts
// (qualified names, fenced declarations, tables with links), which are harder to match
// than a plain list. Each scenario is one such shape.
// =====================================================================================
const onlyOn = (out: Outcome, page: string, count: number): boolean =>
  Array.isArray(out.json?.suggestions) &&
  out.json.suggestions.filter((s) => s.page === page).length === count;
const EVENTS_SRC = `export const MonitorEventType = { JOB: 'job', QUEUE: 'queue', RUN: 'run' } as const;\n`;

{
  // A table keyed to an object inside a namespace re-export, on a page full of common words.
  const page = lines(
    '# Jobs program',
    '',
    '## Close',
    '',
    'Closes a job and returns the deposit to the owner.',
    '',
    '| Name | Type | Description |',
    '| --- | --- | --- |',
    '| `job` | required | The [JobRecord](#job-record) address. |',
    '| `project` | required | The [ProjectRecord](#project-record) address. |',
    '| `owner` | optional | The owning identity. |',
    '| `approver` | optional | The approving identity. |',
  );
  add('audit', 'positive', 'audit-r1', 'a table keyed to an object inside a namespace re-export', {
    files: project({
      'src/index.ts': `${EVENTS_SRC}export * as JobsClient from './jobs.js';\n`,
      'src/jobs.ts': `export const CLOSE_FIELDS = { job: 0, project: 1, owner: 2, approver: 3, projectSettings: 4 };\nexport const LIST_FIELDS = { job: 0, run: 1, archive: 2, rewardsProgram: 3 };\n`,
      'docs/jobs.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'docs/jobs.md',
        line: lineOf(page, '| Name |'),
        target: 'column',
        status: 'drift',
        mentions: 'CLOSE_FIELDS',
        missing: ['projectSettings'],
      }) && onlyOn(out, 'docs/jobs.md', 1),
  });
}

{
  const page = lines(
    '# S3 resources',
    '',
    '| Field | Meaning |',
    '| --- | --- |',
    '| `url` | where the bucket lives |',
    '| `bucket` | its name |',
    '| `IAM` | credentials |',
  );
  add('audit', 'positive', 'audit-r2', 'a table that copies property names four levels down', {
    files: project({
      'src/index.ts': `export const jobSchemas = { version: '3.0', components: { schemas: { S3Resource: { properties: { url: {}, bucket: {}, IAM: {}, allowWrite: {} } }, HFResource: { properties: { repo: {}, revision: {} } } } } };\n`,
      'docs/s3.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'docs/s3.md',
        line: lineOf(page, '| Field |'),
        target: 'column',
        status: 'drift',
        mentions: 'S3Resource',
        missing: ['allowWrite'],
      }),
  });
}

{
  // CORPUS #4 as a docs site tends to write it: an enum declaration in a fence.
  const page = lines(
    '# Error handling',
    '',
    'Every error carries a code:',
    '',
    '```ts',
    'enum ErrorCodes {',
    "  RPC_ERROR = 'RPC_ERROR',",
    "  NO_SESSION = 'NO_SESSION',",
    "  REQUEST_ERROR = 'REQUEST_ERROR',",
    '}',
    '```',
  );
  add(
    'left-to-binding',
    'positive',
    'audit-r3',
    'corpus #4 as written: a fenced enum declaration missing a member',
    {
      files: project({
        'src/index.ts': `export const ErrorCodes = { RPC_ERROR: 'RPC_ERROR', NO_SESSION: 'NO_SESSION', REQUEST_ERROR: 'REQUEST_ERROR', AUTH_ERROR: 'AUTH_ERROR' } as const;\n`,
        'docs/errors.md': page,
      }),
      run: AUDIT,
      expect: (out) =>
        suggests(out, {
          page: 'docs/errors.md',
          line: lineOf(page, '```ts'),
          target: 'block',
          status: 'drift',
          mentions: 'ErrorCodes',
          missing: ['AUTH_ERROR'],
        }),
    },
  );
}

{
  // CORPUS #5 as a docs site tends to write it: qualified names in bold code spans.
  const page = lines(
    '# Configuration',
    '',
    'The SDK supports two networks:',
    '',
    '- **`Network.PRODUCTION`** - production (live traffic)',
    '- **`Network.STAGING`** - development and testing',
  );
  add(
    'left-to-binding',
    'positive',
    'audit-r4',
    'corpus #5 as written: qualified names in a list',
    {
      files: project({
        'src/index.ts': `export const Network = { PRODUCTION: 'production', STAGING: 'staging', LOCAL: 'local' } as const;\n`,
        'docs/configuration.md': page,
      }),
      run: AUDIT,
      expect: (out) =>
        suggests(out, {
          page: 'docs/configuration.md',
          line: lineOf(page, '- **`Network.PRODUCTION`**'),
          target: 'list',
          status: 'drift',
          mentions: 'Network',
          missing: ['LOCAL'],
        }),
    },
  );
}

const STRATEGY_SRC = `export const RunStrategy = { BASIC: 'BASIC', 'BASIC-EXTEND': 'BASIC-EXTEND', TIMED: 'TIMED', ENDLESS: 'ENDLESS' } as const;\n`;
{
  const page = lines(
    '# Strategies',
    '',
    'A run follows one strategy:',
    '',
    '- **BASIC**: runs once and stops.',
    '- **TIMED**: runs on a cron schedule.',
    '- **ENDLESS**: restarts until stopped.',
  );
  add('audit', 'positive', 'audit-r5', 'a list whose items lead with a bold term', {
    files: project({ 'src/index.ts': STRATEGY_SRC, 'docs/strategies.md': page }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'docs/strategies.md',
        line: lineOf(page, '- **BASIC**'),
        target: 'list',
        status: 'drift',
        mentions: 'RunStrategy',
        missing: ['BASIC-EXTEND'],
      }),
  });
}

{
  const page = lines('# Resources', '', 'The resource type is `HF`, `S3` or `URL`.');
  add('left-to-binding', 'positive', 'audit-r6', 'an enumeration written inline in a sentence', {
    files: project({
      'src/index.ts': `export const ResourceType = { HF: 'HF', S3: 'S3', URL: 'URL', OLLAMA: 'Ollama' } as const;\n`,
      'docs/resources.md': page,
    }),
    run: AUDIT,
    expect: (out) =>
      suggests(out, {
        page: 'docs/resources.md',
        line: 3,
        target: 'inline',
        status: 'drift',
        mentions: 'ResourceType',
        missing: ['Ollama'],
      }) && onlyOn(out, 'docs/resources.md', 1),
  });
}

{
  const page = lines(
    '# Options',
    '',
    'Choose `BASIC-EXTEND` to keep the run alive past its timeout.',
  );
  add(
    'left-to-binding',
    'positive',
    'audit-r7',
    'a distinctive value under a key that is not an identifier',
    {
      files: project({ 'src/index.ts': STRATEGY_SRC, 'docs/options.md': page }),
      run: AUDIT,
      expect: (out) =>
        suggests(out, {
          page: 'docs/options.md',
          line: 3,
          target: 'scalar',
          status: 'match',
          mentions: 'RunStrategy',
        }),
    },
  );
}

add('audit', 'positive', 'audit-r8', 'a surface that cannot load is an error, not silence', {
  files: project({
    'src/index.ts': `import 'a-package-that-is-not-installed';\nexport const REGIONS = ['eu', 'us', 'ap'];\n`,
    'docs/regions.md': lines('# Regions', '', '- `eu`', '- `us`'),
  }),
  run: AUDIT,
  expect: (out) =>
    out.code === 1 &&
    /a-package-that-is-not-installed|failed to load/.test(out.stderr + out.stdout),
});

add(
  'audit',
  'negative',
  'audit-rn1',
  'common words in prose that equal an enum value named after them',
  {
    files: project({
      'src/index.ts': EVENTS_SRC,
      'docs/guide.md': lines(
        '# Guide',
        '',
        "A copy runs on the source's project unless `project` is given.",
        '',
        'Hover over `job` to see its type. See the [ProjectRecord](#project-record) for `run` details.',
      ),
    }),
    run: AUDIT,
    expect: (out) => suggestsNothing(out),
  },
);

add('audit', 'negative', 'audit-rn2', 'a table of common words that is not a copy of anything', {
  files: project({
    'src/index.ts': EVENTS_SRC,
    'docs/records.md': lines(
      '# Records',
      '',
      '| Name | Description |',
      '| --- | --- |',
      '| `job` | The [JobRecord](#job-record) address. |',
      '| `owner` | The owning identity. |',
      '| `approver` | The approving identity. |',
      '| `systemProgram` | The system program. |',
    ),
  }),
  run: AUDIT,
  expect: (out) => suggestsNothing(out),
});

add('audit', 'negative', 'audit-rn3', 'a matching list yields one suggestion, not one per item', {
  files: project({
    'src/index.ts': `export const TIERS = ['tier-basic', 'tier-plus', 'tier-max'];\nexport const DEFAULT_TIER = 'tier-basic';\n`,
    'docs/tiers.md': lines(
      '# Tiers',
      '',
      'Records come in:',
      '',
      '- `tier-basic`',
      '- `tier-plus`',
      '- `tier-max`',
    ),
  }),
  run: AUDIT,
  expect: (out) => onlyOn(out, 'docs/tiers.md', 1) && out.json?.suggestions?.[0]?.target === 'list',
});
