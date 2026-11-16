import type { Language } from '../api/adapter.js';
import { description } from './description.js';
import { createSession } from './session.js';
import { docCommentExamples } from './syntax/doc-comments.js';

/** The TypeScript (and JavaScript) language plugin. */
export const typescript: Language = {
  ...description,
  docComments: docCommentExamples,
  createSession,
};
