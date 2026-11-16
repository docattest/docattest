// The machine the smoke test runs on, as the launchers name it.
import process from 'node:process';

export const WINDOWS = process.platform === 'win32';

/** `<os>-<arch>` as the launchers name this machine. */
export const PLATFORM = `${WINDOWS ? 'windows' : process.platform}-${process.arch}`;
