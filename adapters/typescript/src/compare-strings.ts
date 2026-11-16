/** Code-unit string order, independent of locale, so reports are identical everywhere. */
export function compareStrings(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}
