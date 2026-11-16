// What a binary was built for, read from its header, so that a release never packages a
// core for the wrong platform.
import type { Cpu, Os } from '../lib/platforms.ts';

/** What a binary was built for: npm's names for its system and processor. */
export interface BuiltFor {
  os: Os;
  cpu: Cpu | undefined;
}

// Machine numbers: ELF's and PE's, then Mach-O's CPU types.
const MACHINES: Readonly<Record<number, Cpu>> = {
  0x3e: 'x64',
  0xb7: 'arm64',
  0x8664: 'x64',
  0xaa64: 'arm64',
  0x01000007: 'x64',
  0x0100000c: 'arm64',
};

/**
 * What a binary was built for, read from its header: `{ os, cpu }` in npm's names (ELF is
 * `linux`, Mach-O `darwin`, PE `win32`), or undefined for anything else.
 */
export function builtFor(bytes: Buffer): BuiltFor | undefined {
  const cpu = (machine: number): Cpu | undefined => MACHINES[machine];
  const elf = bytes.readUInt32BE(0) === 0x7f454c46;
  if (elf) return { os: 'linux', cpu: cpu(bytes.readUInt16LE(18)) };
  const macho = bytes.readUInt32LE(0) === 0xfeedfacf;
  if (macho) return { os: 'darwin', cpu: cpu(bytes.readUInt32LE(4)) };
  if (bytes.toString('latin1', 0, 2) !== 'MZ') return undefined;
  const pe = bytes.readUInt32LE(0x3c);
  if (bytes.toString('latin1', pe, pe + 4) !== 'PE\0\0') return undefined;
  return { os: 'win32', cpu: cpu(bytes.readUInt16LE(pe + 4)) };
}
