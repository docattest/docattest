import { createProject, type ProjectInput } from 'docattest';

export const CONFIG = JSON.stringify({
  pages: ['docs/**/*.md', 'README.md'],
  surfaces: { default: './src/index.ts' },
  adapter: 'typescript',
});

/** An in-memory project with the standard config, a surface and some pages. */
export function project(files: Record<string, string>, env?: ProjectInput['env']) {
  return createProject({ files: { 'docattest.config.json': CONFIG, ...files }, env });
}
