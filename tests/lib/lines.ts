// Text written line by line, as the suites' fixtures write files and pages.

/** The lines, each ended by a newline. */
export const lines = (...parts: string[]): string => `${parts.join('\n')}\n`;
