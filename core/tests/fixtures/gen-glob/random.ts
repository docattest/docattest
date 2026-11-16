// The seeded PRNG of gen-glob.ts (GLOB_CASES_SEED), and how many random cases it makes
// (GLOB_CASES_SCALE).
// --- deterministic randomness ------------------------------------------------------------
let seed = Number(process.env.GLOB_CASES_SEED ?? 0x5eed1234);
export const scale = Number(process.env.GLOB_CASES_SCALE ?? 1);
export const rand = (): number => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = seed;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
export const int = (n: number): number => Math.floor(rand() * n);
export function pick<T>(a: readonly T[]): T {
  const x = a[int(a.length)];
  if (x === undefined) throw new Error('pick from an empty list');
  return x;
}
