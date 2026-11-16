/**
 * A problem with the project or the request, such as an invalid config or an unknown page.
 * The message is complete on its own; the command line prints it without a stack.
 */
export class UserError extends Error {
  override readonly name = 'UserError';

  constructor(message: string) {
    super(`docattest: ${message}`);
  }
}

/** A broken invariant inside docattest: a bug to report, never the user's mistake. */
export class InternalError extends Error {
  override readonly name = 'InternalError';

  constructor(message: string, options?: ErrorOptions) {
    super(`docattest internal error: ${message}`, options);
  }
}

/** The message of a thrown value, whatever was thrown. */
export function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
