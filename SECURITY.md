# Security

## What docattest runs

docattest executes code. Checking a project runs, with the permissions of whoever runs it:

- every `check` example in its documentation pages, and in the doc comments of its source
  files under `sources`;
- the expression of every fact, which loads the project's public API, and so the project's
  own code and its dependencies;
- the project's setup module (the runners of cases tables and named facts);
- for TypeScript, the project's own TypeScript compiler.

This is by design: it is how a page proves what it says, as a test suite does. Treat
`docattest check` like your test suite. In particular, **running it on a pull request from
someone you do not trust runs their code**: give that job no secrets and no write access,
as you would a test job.

Examples run in separate workers with a time limit, and with internet access refused unless
a block is tagged `network` and `DOCATTEST_NETWORK=1` is set. These are guards against
accidents, not a sandbox: the network guard does not stop native code, and nothing limits
what attested code may do to the file system.

Other things that reach outside the project:

- `docattest refresh` fetches snapshot data over HTTP(S) from the URLs the project's
  snapshot files name. No other command uses the network.
- `docattest init` writes `docattest.config.json`; `init --hooks` writes commands into a
  coding agent's settings in the project, `init --agents` updates `AGENTS.md`,
  `init --git-hook` writes a git pre-commit hook, and `init --skill --global` writes into
  your home directory. Each prints what it wrote.
- The launchers run the core named by `DOCATTEST_CORE` when it is set.

## Supported versions

The latest release receives fixes. docattest is pre-1.0, so a fix is released as a new
patch or minor version, not backported.

## Reporting a vulnerability

Please do not open a public issue. Report it privately through the repository's security
advisories (the "Report a vulnerability" button on the Security tab,
`<REPOSITORY_URL>/security/advisories/new`), with what you found, how to reproduce it, and
which version and platform you used. The maintainer will acknowledge it and keep you
informed; when a fix is released, the advisory is published with credit to you unless you
ask otherwise.

A report that docattest runs code from a project's documentation is not a vulnerability:
that is what it is for (see above). A way to make docattest run code a user did not ask it
to run, or reach the network or files outside what is described here, is.
