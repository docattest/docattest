#!/usr/bin/env node
// Refuse to publish a commit that CI has not passed: run by the release workflow before it
// publishes.
//
//   node scripts/release/ci-passed.ts <commit>
//
// The release builds and publishes from a tag, and CI runs on pushes to main, so nothing else
// stops a tag on a commit whose CI failed from being published. It waits while CI is still
// running the commit, up to an hour, and reads GITHUB_REPOSITORY and GITHUB_TOKEN as the
// workflow sets them.
import process from 'node:process';
import { setTimeout as sleep } from 'node:timers/promises';
import { fail } from '../lib/fail.ts';
import { type CiRun, type CiVerdict, ciVerdict } from './ci-runs.ts';

/** How long CI may still be running the commit, and how often it is asked again. */
const WAIT_MS = 60 * 60 * 1000;
const POLL_MS = 30 * 1000;

const commit = process.argv[2];
const repository = process.env.GITHUB_REPOSITORY;
const token = process.env.GITHUB_TOKEN;
if (!commit || !repository || !token) {
  fail('usage: GITHUB_REPOSITORY=<owner/name> GITHUB_TOKEN=<token> ci-passed.ts <commit>');
}

const verdict = await waitForVerdict(commit, repository, token);
if (verdict !== 'passed') fail(refusal(commit, verdict));
process.stdout.write(`CI passed ${commit}\n`);

// Ask until the runs say passed or failed, or the wait is over.
async function waitForVerdict(sha: string, repo: string, auth: string): Promise<CiVerdict> {
  const deadline = Date.now() + WAIT_MS;
  for (;;) {
    const verdict = ciVerdict(await ciRuns(sha, repo, auth));
    if (verdict === 'passed' || verdict === 'failed' || Date.now() >= deadline) return verdict;
    process.stdout.write(
      `CI has ${verdict === 'none' ? 'not started on' : 'not finished'} ${sha}; waiting\n`,
    );
    await sleep(POLL_MS);
  }
}

// The runs of the CI workflow on one commit.
async function ciRuns(sha: string, repo: string, auth: string): Promise<CiRun[]> {
  const url = `https://api.github.com/repos/${repo}/actions/workflows/ci.yml/runs?head_sha=${sha}&per_page=50`;
  const answer = await fetch(url, {
    headers: { authorization: `Bearer ${auth}`, accept: 'application/vnd.github+json' },
  });
  if (!answer.ok) fail(`GitHub answered ${answer.status} when asked for the CI runs of ${sha}`);
  const body = (await answer.json()) as { workflow_runs: CiRun[] };
  return body.workflow_runs;
}

function refusal(sha: string, verdict: CiVerdict): string {
  const why = {
    failed: 'CI failed on it',
    running: 'CI was still running it after an hour',
    none: 'CI never ran it: push the commit to main before tagging it',
  }[verdict as Exclude<CiVerdict, 'passed'>];
  return `not publishing ${sha}: ${why}. Fix main, then tag a commit CI passed.`;
}
