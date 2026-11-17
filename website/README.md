# website

The docattest website: a home page that says what docattest does, and a playground where a
visitor breaks a small invented project and watches docattest catch it. It is built with
[Vite](https://vite.dev) and plain TypeScript into static files, and
[`.github/workflows/pages.yml`](../.github/workflows/pages.yml) deploys them to GitHub Pages
from `main`.

```sh
npm run dev --workspace website        # serve with live reload
npm run build --workspace website      # type-check, then build into website/dist
npm run preview --workspace website    # serve the build
npm test --workspace website
```

The home page (`index.html`) keeps each section below the hero in its own file under
`src/home/sections/`, written into the page where it says `<!-- include: … -->`. The dev
server does not watch those files: reload the page after editing one.

## How the playground stays honest

Every line the playground's terminal prints was printed by the docattest built from this
repository. The recorder (`recorder/`) creates the invented project, runs each stop's
commands in a fresh copy for every setting of the stop's switches (the yellow spots a
visitor clicks), and writes the output to `src/playground/recordings.json`, which the page
replays. That file is generated: never edit it by hand.

After a change to docattest's output, or to the recorder, record again and commit the result:

```sh
npm run build:core && npm run build    # the core and the npm package the recorder runs
npm run record --workspace website
```

CI runs `npm run record:check --workspace website`, which fails when the committed recordings
are not what docattest prints today.

The version the pages name (`%DOCATTEST_VERSION%` in the HTML) is read from the npm package's
manifest at build time, so the site never names a version the repository has moved past.

## How search engines and link previews see it

They need absolute addresses, so `SITE_URL` in `vite.config.ts` names where the site is
served, and everything that points at a page starts from it: the canonical links and
social-card tags (`%SITE_URL%` in the HTML) and the `sitemap.xml` the build writes from the
list of pages. When the site moves to its own domain, change `SITE_URL` and nothing else. No
`robots.txt` is written, because crawlers read one only at a domain's root, which a GitHub
Pages project path is not. The card a shared link shows is `public/img/social-card.png`,
1200 × 630.

## How to add a playground stop

1. In `recorder/stops.ts`, add the stop: its switches (each one's first option must occur
   exactly once in its file), its commands, and any files a command writes that the page
   should show.
2. In `src/playground/stops.ts`, add what the page says there, with the same `id` and the
   same command ids, in the order the stops should be listed.
3. Record again (above). The tests (`test/playground/stops.test.ts`) fail until the two lists
   agree and every command has a recording in every setting.
