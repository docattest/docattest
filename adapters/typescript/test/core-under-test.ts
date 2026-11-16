// The docattest core the tests run directly: the one `scripts/with-core.ts`, which runs
// `npm test`, names in DOCATTEST_CORE.

/** The core's path; fails when the tests were not started through `scripts/with-core.ts`. */
export function coreUnderTest(): string {
  const core = process.env.DOCATTEST_CORE;
  if (!core) {
    throw new Error('DOCATTEST_CORE is not set: run the tests with scripts/with-core.ts');
  }
  return core;
}
