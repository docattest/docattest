// From what the probe found to observables: each value with the fact expression that reads
// it, written the way a person would write it in a directive.
import type { Observable, Observation } from '../../api/adapter.js';
import type { Fields, ProbeNode } from './probe.js';

const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
/** Rows with more fields than this are not read as pairs of fields. */
const MAX_PAIRED_FIELDS = 6;

type Planned = Omit<Observable, 'surface'>;

/** A member access: `.key`, or `['key']` when the key is not an identifier. */
export function member(key: string): string {
  if (IDENTIFIER.test(key)) return `.${key}`;
  return /['\\\n]/.test(key) ? `[${JSON.stringify(key)}]` : `['${key}']`;
}

// A directive cannot hold `-->`, and a line break would split it.
const writable = (key: string) => !key.includes('-->') && !/[\r\n]/.test(key);

/** Every observable of one export, from the nodes the probe returned for it. */
export function planExport(name: string, nodes: readonly ProbeNode[]): Planned[] {
  return nodes.flatMap((node) => {
    if (!node.path.every(writable)) return [];
    const at = name + node.path.map(member).join('');
    const path = [name, ...node.path];
    if (node.kind === 'scalar') return scalar(at, path, node);
    if (node.kind === 'list') return [list(at, path, node.items, node.array)];
    if (node.kind === 'records') return fieldPlan(path, node.level, node.fields, at);
    return objectPlan(at, path, node);
  });
}

function scalar(
  expression: string,
  path: string[],
  node: Extract<ProbeNode, { kind: 'scalar' }>,
): Planned[] {
  const value: Observation = { text: node.text, description: `${node.type} ${node.text}` };
  if (node.type === 'string') value.kind = 'text';
  return [{ expression, path, value }];
}

function list(expression: string, path: string[], items: string[], array = true): Planned {
  // As `observe` reports an array or a set, so the value is what `check` will see.
  const text = array ? items.join(',') : '[object Set]';
  const value = { text, items, description: array ? 'an array' : 'an object' };
  return { expression, path, value };
}

function objectPlan(
  at: string,
  path: string[],
  node: Extract<ProbeNode, { kind: 'object' }>,
): Planned[] {
  if (node.enum) {
    // The reverse mapping adds the numbers as keys; the names are what a page lists.
    const keys = (node.keys ?? []).filter((k) => Number.isNaN(Number(k)));
    return [list(`Object.keys(${at}).filter((k) => Number.isNaN(Number(k)))`, path, keys)];
  }
  const out: Planned[] = [];
  if (node.keys) out.push(list(`Object.keys(${at})`, path, node.keys));
  if (node.values) out.push(list(`Object.values(${at})`, path, node.values));
  out.push(...fieldPlan(path, node.level, node.fields ?? {}, `Object.values(${at})`));
  return out;
}

// Each field of the rows near the top of an export, and at the top each ordered pair of
// fields joined by a space, the way a table writes `GET /jobs` for `{ method, path }`.
function fieldPlan(path: string[], level: number, rowFields: Fields, rows: string): Planned[] {
  const fields = Object.entries(rowFields).filter(([f]) => writable(f));
  if (level > 1) return [];
  const single = fields.map(([f, values]) =>
    list(`${rows}.map((r) => r${member(f)})`, [...path, f], values),
  );
  if (level > 0 || fields.length > MAX_PAIRED_FIELDS) return single;
  const pairs = fields.flatMap(([a, as]) =>
    fields
      .filter(([b]) => b !== a)
      .map(([b, bs]) => {
        const expression = `${rows}.map((r) => \`\${r${member(a)}} \${r${member(b)}}\`)`;
        const items = as.map((x, k) => `${x} ${bs[k] ?? ''}`);
        return list(expression, [...path, a, b], items);
      }),
  );
  return [...single, ...pairs];
}
