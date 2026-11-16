# scripts

Tools for developing and releasing docattest. Each is TypeScript that Node (22.18 or later)
runs as it is, with no dependencies, and says at its top what it does and how to run it.

The top level holds the development tools the npm scripts run: running a command against
the core built in this checkout (so the npm package's launcher and API use it), checking
that every place a release's version is written agrees (and setting it everywhere), and
checking every relative link in the repository's Markdown. `npm run versions` and
`npm run links` run in CI.

[`release/`](release) holds what the release workflow runs: packaging the core for one
platform as an npm package, packaging the Go archive for one platform, and refusing to
publish the `docattest` npm package before its platform packages are on the registry.

What several scripts, and the test suites, need is in [`lib/`](lib), one small typed module
for each thing: the repository's root and its JSON files, the platforms of
`protocol/platforms.json`, the version of `protocol/core.json`, reading a command line with
Node's own parser, and stopping with a message. What only the release scripts share (running
another program, reading which platform a binary was built for) is beside them in
`release/`.

## How to add a script

1. Write it in TypeScript in the area it belongs to, with a comment at its top saying what
   it does and how to run it.
2. Put the code it shares with other scripts in a module named after what that code is,
   never in a grab-bag.
3. If developers run it often, add an npm script; if CI should run it, add it to
   [ci.yml](../.github/workflows/ci.yml) and the checks table in
   [CONTRIBUTING.md](../CONTRIBUTING.md#checks).

Building the Python wheels is the Python package's own script,
[adapters/python/build_wheel.py](../adapters/python/build_wheel.py), and the install smoke
test is a test suite, [tests/smoke/](../tests/README.md#smoke-a-fresh-install).
[CONTRIBUTING.md](../CONTRIBUTING.md), "Releasing", puts them together.
