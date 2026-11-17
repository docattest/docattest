import { readFileSync } from 'node:fs';
import { fileURLToPath, URL } from 'node:url';
import { defineConfig, type Plugin } from 'vite';

const local = (path: string): string => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  // Relative URLs, so the build works wherever it is served: a project's GitHub Pages path
  // (/docattest/), a custom domain, or `vite preview`.
  base: './',
  plugins: [htmlIncludes(), docattestVersion()],
  build: {
    rolldownOptions: {
      input: { home: local('index.html'), playground: local('playground.html') },
    },
  },
});

/** Writes a file's HTML where a page says <!-- include: path -->, the path from website/, so
 *  the long home page is kept one section to a file. The dev server does not watch these
 *  files: reload the page after editing one. */
function htmlIncludes(): Plugin {
  const include = /<!-- include: ([\w./-]+) -->/g;
  return {
    name: 'html-includes',
    transformIndexHtml: {
      order: 'pre',
      handler: (html) =>
        html.replace(include, (_line, path: string) => readFileSync(local(path), 'utf8')),
    },
  };
}

/** Writes the npm package's version where a page says %DOCATTEST_VERSION%, so the site never
 *  names a version the repository has moved past. */
function docattestVersion(): Plugin {
  const manifest: unknown = JSON.parse(
    readFileSync(local('../adapters/typescript/package.json'), 'utf8'),
  );
  if (
    typeof manifest !== 'object' ||
    manifest === null ||
    !('version' in manifest) ||
    typeof manifest.version !== 'string'
  ) {
    throw new Error('adapters/typescript/package.json has no version');
  }
  const { version } = manifest;
  return {
    name: 'docattest-version',
    transformIndexHtml: (html) => html.replaceAll('%DOCATTEST_VERSION%', version),
  };
}
