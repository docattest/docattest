export const RATE = 3;
export const quote = (kg: number) => kg * RATE;
export const internalCache = new Map<string, number>();
export const ProgramName = 'shop';
