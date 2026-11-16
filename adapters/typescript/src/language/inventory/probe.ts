// The probe `audit` evaluates once per export: one fact expression that walks the export's
// plain objects and namespaces breadth first and returns, as JSON, every value a page could
// have copied from it. The values are only a guide: whatever is suggested is evaluated again,
// through its own expression, before it is reported.
import { isObject, isStringArray, objectOrNothing, type JsonObject } from '../../json.js';

/** Property hops below an export the walk goes, namespaces included. */
export const MAX_DEPTH = 6;
/** Values one export may contribute, shallowest first. */
export const MAX_NODES = 400;
/** Members of one object the walk descends into. */
export const MAX_MEMBERS = 100;
/** Longer lists, key sets and row sets are not read: no page copies them by hand. */
export const MAX_ITEMS = 500;

/** What every value the walk found has: `path`, the keys from the export to it. */
interface Found {
  path: string[];
  /** Plain objects, namespaces not counted, on the way from the export to this value. */
  level: number;
}

/** Each scalar field every row has (rows of an array, or values of an object), by row. */
export type Fields = Record<string, string[]>;

/** One value the walk found. */
export type ProbeNode =
  /** A scalar: its `typeof` and `String(value)`. */
  | (Found & { kind: 'scalar'; type: string; text: string })
  /** A list: its items as strings, and whether it is an array (not a set). */
  | (Found & { kind: 'list'; items: string[]; array: boolean })
  /**
   * An object: its keys, its values when every value is a scalar, whether it is a numeric
   * TypeScript enum (whose keys include the reverse mapping), and its rows' fields.
   */
  | (Found & { kind: 'object'; keys?: string[]; values?: string[]; enum: boolean; fields?: Fields })
  /** Rows of an array: their fields. */
  | (Found & { kind: 'records'; fields: Fields });

// Helpers are parameters, so they are never mistaken for setup facts or clash with a surface
// export, and the walk reads only what `check` could read with a plain expression.
const PROBE = `((v,
  prim = (x) => ['string', 'number', 'bigint', 'boolean'].includes(typeof x),
  plain = (x) => typeof x === 'object' && x !== null && !Array.isArray(x) &&
    (Object.getPrototypeOf(x) === null ||
      Object.getPrototypeOf(Object.getPrototypeOf(x)) === null),
  space = (x) => Object.prototype.toString.call(x) === '[object Module]' ||
    Object.hasOwn(x, '__esModule'),
  rowsOf = (x) => Array.isArray(x) ? x : Object.values(x),
  fieldsOf = (rows) => {
    if (rows.length === 0 || rows.length > ${MAX_ITEMS} || !rows.every(plain)) return undefined;
    const out = {};
    for (const f of Object.keys(rows[0])) {
      const ok = rows.every((r) => Object.hasOwn(r, f) && prim(r[f]));
      if (ok) out[f] = rows.map((r) => String(r[f]));
    }
    return Object.keys(out).length ? out : undefined;
  },
  describe = (x, path, level) => {
    if (prim(x)) return { path, kind: 'scalar', type: typeof x, text: String(x), level };
    if (Array.isArray(x) || x instanceof Set) {
      const items = [...x];
      if (items.length <= ${MAX_ITEMS} && items.every(prim)) {
        return { path, kind: 'list', items: items.map(String), array: Array.isArray(x), level };
      }
      const fields = Array.isArray(x) ? fieldsOf(items) : undefined;
      return fields ? { path, kind: 'records', fields, level } : undefined;
    }
    if (!plain(x)) return undefined;
    const keys = Object.keys(x);
    const vals = keys.map((k) => x[k]);
    return { path, kind: 'object', level, namespace: space(x),
      keys: keys.length <= ${MAX_ITEMS} ? keys : undefined,
      values: keys.length <= ${MAX_ITEMS} && vals.every(prim) ? vals.map(String) : undefined,
      enum: keys.some((k) => typeof x[k] === 'number' && x[x[k]] === k),
      fields: vals.length && vals.every(plain) ? fieldsOf(rowsOf(x)) : undefined };
  },
  walk = () => {
    const out = [];
    const seen = new Set();
    const queue = [[v, [], 0]];
    while (queue.length && out.length < ${MAX_NODES}) {
      const [x, path, level] = queue.shift();
      if (typeof x === 'object' && x !== null) {
        if (seen.has(x)) continue;
        seen.add(x);
      }
      let node;
      try { node = describe(x, path, level); } catch { node = undefined; }
      if (!node) continue;
      out.push(node);
      if (node.kind !== 'object' || path.length >= ${MAX_DEPTH}) continue;
      const below = node.namespace ? level : level + 1;
      for (const key of Object.keys(x).slice(0, ${MAX_MEMBERS})) {
        try { queue.push([x[key], [...path, key], below]); } catch { continue; }
      }
    }
    return out;
  }) => JSON.stringify(walk()))`;

/** The probe expression for one export. */
export function probeExpression(name: string): string {
  return `${PROBE}(${name})`;
}

// A helper that takes arguments is never called. One that returns a promise is awaited, and
// says so, since a fact must then await it before reading below its value.
const HELPER_PROBE = `((h, probe) => typeof h !== 'function' || h.length !== 0 ? 'null' :
  ((r) => typeof r?.then === 'function'
    ? r.then((v) => '{"awaited":true,"nodes":' + probe(v) + '}')
    : '{"awaited":false,"nodes":' + probe(r) + '}')(h()))`;

/** The probe expression for a named fact of the setup module. */
export function helperProbeExpression(name: string): string {
  return `${HELPER_PROBE}(${name}, ${PROBE})`;
}

/** What a helper's probe found, or undefined when it was not called. */
export interface HelperProbe {
  /** It returned a promise. */
  awaited: boolean;
  nodes: ProbeNode[];
}

/** The probe's JSON, or undefined when it is not what the probe returns. */
export function parseProbe(text: string): ProbeNode[] | undefined {
  return toNodes(parseJson(text));
}

/** A helper probe's JSON, or undefined when the helper was not called. */
export function parseHelperProbe(text: string): HelperProbe | undefined {
  const parsed = objectOrNothing(text);
  if (parsed === undefined || typeof parsed.awaited !== 'boolean') return undefined;
  const nodes = toNodes(parsed.nodes);
  return nodes && { awaited: parsed.awaited, nodes };
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return undefined;
  }
}

function toNodes(parsed: unknown): ProbeNode[] | undefined {
  return Array.isArray(parsed) ? parsed.flatMap(toNode) : undefined;
}

/** A node the probe printed, or none when it does not have its kind's fields. */
function toNode(entry: unknown): ProbeNode[] {
  if (!isObject(entry) || !isStringArray(entry.path) || typeof entry.level !== 'number') return [];
  const found: Found = { path: entry.path, level: entry.level };
  const node = KIND_READERS[String(entry.kind)]?.(entry, found);
  return node ? [node] : [];
}

/** For each kind, the node an entry of it makes, or undefined when it lacks a field. */
const KIND_READERS: Record<string, (entry: JsonObject, found: Found) => ProbeNode | undefined> = {
  scalar: (entry, found) =>
    typeof entry.type === 'string' && typeof entry.text === 'string'
      ? { ...found, kind: 'scalar', type: entry.type, text: entry.text }
      : undefined,
  list: (entry, found) =>
    isStringArray(entry.items)
      ? { ...found, kind: 'list', items: entry.items, array: entry.array === true }
      : undefined,
  records: (entry, found) => {
    const fields = isObject(entry.fields) ? toFields(entry.fields) : undefined;
    return fields ? { ...found, kind: 'records', fields } : undefined;
  },
  object: (entry, found) => {
    const node: ProbeNode = { ...found, kind: 'object', enum: entry.enum === true };
    if (isStringArray(entry.keys)) node.keys = entry.keys;
    if (isStringArray(entry.values)) node.values = entry.values;
    const fields = isObject(entry.fields) ? toFields(entry.fields) : undefined;
    if (fields) node.fields = fields;
    return node;
  },
};

function toFields(raw: Record<string, unknown>): Fields | undefined {
  const out: Fields = {};
  for (const [name, values] of Object.entries(raw)) if (isStringArray(values)) out[name] = values;
  return Object.keys(out).length ? out : undefined;
}
