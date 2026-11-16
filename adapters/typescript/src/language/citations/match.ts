import type { DeclaredTest } from '../../api/adapter.js';
import type { TestAttestation } from '../../api/attestation.js';

/** What a citation names: a title, and the enclosing describe titles when given. */
export type CitedPath = Pick<TestAttestation, 'title' | 'describes'>;

/**
 * Whether a declared test or describe block is the one cited: same title and, when the
 * citation names describe blocks, exactly the same enclosing describe titles.
 */
export function matchesCitation(declared: DeclaredTest, cited: CitedPath): boolean {
  if (declared.title !== cited.title) return false;
  if (!cited.describes.length) return true;
  const { describes } = declared;
  return (
    describes.length === cited.describes.length &&
    describes.every((d, k) => d === cited.describes[k])
  );
}
