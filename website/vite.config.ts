import { readFileSync } from 'node:fs';
import { fileURLToPath, URL } from 'node:url';
import { defineConfig, type Plugin } from 'vite';

const local = (path: string): string => fileURLToPath(new URL(path, import.meta.url));

/** Where the site is served. Search engines and link previews need absolute addresses, so the
 *  pages' canonical links, social cards and the sitemap are all built from this one. */
const SITE_URL = 'https://docattest.github.io/docattest/';

/** Each page: the name of its build input, its source file, and its address under SITE_URL. */
const PAGES = [
  { input: 'home', file: 'index.html', address: '' },
  { input: 'playground', file: 'playground.html', address: 'playground.html' },
] as const;

export default defineConfig({
  // Relative URLs, so the build works wherever it is served: a project's GitHub Pages path
  // (/docattest/), a custom domain, or `vite preview`.
  base: './',
  plugins: [
    htmlIncludes(),
    pageValues({ DOCATTEST_VERSION: docattestVersion(), SITE_URL }),
    sitemap(),
  ],
  build: {
    rolldownOptions: {
      input: Object.fromEntries(PAGES.map((page) => [page.input, local(page.file)])),
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

/** Writes each value where a page says %NAME%. It runs before Vite reads the page's URLs, so
 *  an address built from %SITE_URL% is already absolute when Vite sees it. */
function pageValues(values: Record<string, string>): Plugin {
  return {
    name: 'page-values',
    transformIndexHtml: {
      order: 'pre',
      handler: (html) =>
        Object.entries(values).reduce(
          (page, [name, value]) => page.replaceAll(`%${name}%`, value),
          html,
        ),
    },
  };
}

/** The npm package's version, so the site never names a version the repository has moved
 *  past. */
function docattestVersion(): string {
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
  return manifest.version;
}

/** Writes sitemap.xml, listing every page, for submitting the site to search engines. */
function sitemap(): Plugin {
  const urls = PAGES.map((page) => `  <url><loc>${SITE_URL}${page.address}</loc></url>`);
  const source = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ...urls,
    '</urlset>',
    '',
  ].join('\n');
  return {
    name: 'sitemap',
    apply: 'build',
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: 'sitemap.xml', source });
    },
  };
}
