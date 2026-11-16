const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/**
 * For each generated line, the first original line it maps to (0-based), from a source
 * map's `mappings`. Only lines are needed: failures are reported as `block line N`.
 */
export function decodeLines(mappings: string): (number | undefined)[] {
  const out: (number | undefined)[] = [];
  let source = 0;
  for (const line of mappings.split(';')) {
    let first: number | undefined;
    for (const segment of line.split(',')) {
      if (!segment) continue;
      const fields = decodeVlq(segment);
      const delta = fields[2];
      if (fields.length < 4 || delta === undefined) continue;
      source += delta;
      first ??= source;
    }
    out.push(first);
  }
  return out;
}

/** Decode one base64 VLQ segment into its signed fields. */
export function decodeVlq(segment: string): number[] {
  const out: number[] = [];
  let value = 0;
  let shift = 0;
  for (const char of segment) {
    const digit = BASE64.indexOf(char);
    value += (digit & 31) << shift;
    if (digit & 32) {
      shift += 5;
      continue;
    }
    out.push(value & 1 ? -(value >>> 1) : value >>> 1);
    value = 0;
    shift = 0;
  }
  return out;
}
