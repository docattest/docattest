import { messageOf } from '../../api/errors.js';

/**
 * Errors attested code leaves behind: unhandled rejections and exceptions from timers.
 * While an item runs they belong to it; otherwise they go to whoever is listening.
 */
export class Strays {
  private current: string[] | undefined;

  constructor(private readonly outside: (message: string) => void) {}

  /** Record an escaped error. */
  add(reason: unknown): void {
    const message = messageOf(reason);
    if (this.current) this.current.push(message);
    else this.outside(message);
  }

  /**
   * Run one item, then give its escaped errors a turn of the event loop to surface: a
   * rejection nobody awaited is reported after the microtasks of the block drain.
   */
  async during<T>(run: () => Promise<T>): Promise<{ value: T; strays: string[] }> {
    const mine: string[] = [];
    this.current = mine;
    try {
      const value = await run();
      await new Promise((resolve) => setImmediate(resolve));
      return { value, strays: mine };
    } finally {
      this.current = undefined;
    }
  }
}
