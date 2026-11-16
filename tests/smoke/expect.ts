// The smoke test's assertions: each one that holds is shown, so the log reads as a record.
import process from 'node:process';

/** Fail the smoke test unless `condition` holds. */
export function expect(condition: boolean, message: string): void {
  if (!condition) throw new Error(`expected: ${message}`);
  process.stdout.write(`✓ ${message}\n`);
}
