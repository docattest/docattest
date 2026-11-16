const METHODS = [
  ...['log', 'info', 'warn', 'error', 'debug', 'trace', 'dir', 'table'],
  ...['group', 'groupEnd', 'groupCollapsed', 'time', 'timeEnd', 'timeLog', 'count', 'assert'],
];

/** A console that prints nothing: attested code may log, and must not mix with the report. */
export const QUIET_CONSOLE: Console = {
  ...console,
  ...Object.fromEntries(METHODS.map((m) => [m, () => undefined])),
};
