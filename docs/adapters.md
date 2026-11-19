---
title: Adapters
---

# Adapters

The core (the Rust program in `core/`, docattest's only implementation) parses pages,
binds facts to targets, loads snapshots and manifests, builds the index and writes the
report. It never reads source code and knows no language. Everything that needs one is
asked of an adapter: a separate program for one language, which the core starts and talks
to over a small line-based protocol. This package's adapter is the TypeScript one; the
Python, Go and Rust packages each bring their own.

The CLI exposes these commands, whichever adapter serves a project: <!-- fact: COMMANDS -->
- `check`
- `where`
- `uncovered`
- `index`
- `explain`
- `refresh`
- `skill`
- `audit`
- `inventory`
- `impact`
- `mutate`
- `hooks`
- `agents`
- `conformance`

## What an adapter answers

An adapter first describes its language without loading a compiler: which info-string
languages its blocks use, which files are source code, which names are never the project's
own, and its own words for what the core's sentences name. It then answers questions about
text that need no project (whether an example asserts anything, what a doc comment says,
which symbol a code span names, what shape a fact expression has), and, once a project is
open, the questions that do: compiling and running examples, evaluating facts, which
symbols an attestation exercises, what a surface exports, the tests a file declares, the
values a surface offers to `audit`, and what a change affects for `impact`. Once per check
it may also report what it finds wrong outside every attestation, such as a linter's
finding in a file that only docattest builds, which fails the check at its file and line,
and warnings about the whole project ([the report](report.md#what-the-adapter-finds-outside-every-proof-fails-at-its-line)).

[protocol](protocol.md) shows that conversation with this package's adapter. The protocol
itself, every method and every rule, is
[protocol/PROTOCOL.md](../protocol/PROTOCOL.md).

## The shapes are published, and any adapter can be held to them

Everything that crosses the boundary has a JSON Schema in `protocol/schema/`, and the
package ships a copy. A suite the core carries (`docattest conformance run`) drives an adapter
in any language through the protocol and checks its behaviour and every line it writes. What the core
prints is further held to recorded outputs, byte for byte. Adding a language is a new
adapter program, not a change to the core: [adapters/README.md](../adapters/README.md) says
how.

## Why

A language that only has to answer questions, in its own language, about its own code, can
arrive without touching the parser, the binder or the report. If an adapter needs more,
the core is missing something, not the protocol.
