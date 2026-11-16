// The clock every recorded case runs at.

/**
 * The environment every recorded case runs in: the clock of the recordings (the core reads
 * DOCATTEST_NOW; snapshot ages and `refresh`'s `fetchedAt` depend on it) and one time zone.
 */
export const clockEnv = (recordedAt: string): { DOCATTEST_NOW: string; TZ: string } => ({
  DOCATTEST_NOW: recordedAt,
  TZ: 'UTC',
});
