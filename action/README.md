# action

The docattest GitHub Action. It runs `docattest check --json` in a project, writes each
result that fails or errors, and each finding of `--base`, as an annotation on its page's
line, so a pull request shows the stale sentence where it is written, and writes the counts
and the findings to the run's summary page. It exits as docattest did.

Its metadata is [`action.yml`](../action.yml) at the repository's root, because GitHub
Marketplace lists an action only from there. GitHub runs [`main.ts`](main.ts) on its Node 24
runtime, which strips the types; nothing is built.

## Using it

The action runs the docattest the project installed, so a workflow installs docattest as the
[Install](../README.md#install) section says before this step: the npm package (found in
`node_modules/.bin`, as npx finds it) or any install on the `PATH`. Pin the action to a
docattest release's tag.

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0 # only needed for --base
- uses: actions/setup-node@v4
  with:
    node-version: 22
- run: npm ci
- uses: docattest/docattest@v0.1.5
  with:
    args: --base origin/main
```

| input | default | what |
|---|---|---|
| `working-directory` | `.` | the project's root, where `docattest.config.json` is, from the repository's root |
| `args` | | more arguments for `docattest check`, written as on a command line |

## Changing it

The action decides nothing about what passes: the report is docattest's, and the action
only places it. [`findings.ts`](findings.ts) chooses what needs attention,
[`annotation.ts`](annotation.ts) and [`summary.ts`](summary.ts) write it for GitHub, and
[`runCheck.ts`](runCheck.ts) runs docattest. Their tests are in [`test/`](test)
(`npm run test:action`), and CI runs the action itself on this repository's docs.
