# Acceptance criteria

What any solution to documentation drift must satisfy, for projects of different shapes:
a small service with a README, a monorepo whose tests cite its specification by section,
and a public SDK with a documentation site. The drift events it is judged against are in
[CORPUS.md](CORPUS.md).

## The problem

Code is increasingly written by agents from natural-language context, which makes docs
and comments load-bearing inputs. Prose cannot be run, so it drifts from the code it
describes, and drift now propagates into generated code. We want prose that CI holds to
the standard of code: when behaviour changes, the prose that describes it changes, or the
build fails.

## Detection

1. **Stale fact fails CI.** A value, list member, name, type shape or signature stated in
   a living doc that no longer matches the code fails the build, naming file, line and
   discrepancy.
2. **Stale behaviour fails CI.** A behaviour a living doc describes with an executable
   expectation fails the build at the doc location when it no longer holds.
3. **Broken samples fail CI.** A sample that does not compile fails. Where a runtime
   target exists, a sample that fails at runtime fails.
4. **External facts are pinned.** A statement about an outside system is tied to a
   committed snapshot; disagreement fails; one command refreshes; the refresh is a
   reviewable diff.
5. **Comments count.** The same mechanism applies to doc comments on code.

## Noise

6. **Refactors are silent.** A change that keeps tests green and alters no behaviour
   produces zero doc failures. Replaying history, firings with no subsequent doc change
   must be near zero. (Baseline to beat: a test-hash approach measured 268 firings and 0
   subsequent doc changes on one real history.)
7. **Every failure is fixable at the failure site.** Expected versus actual plus the doc
   line. Resolution is editing the doc or the code, never acknowledging elsewhere. No
   lockfile, no update command.

## Authoring

8. **Docs stay docs.** Ordinary prose, examples and tables; nothing system-specific in
   published output; a checked statement costs no more than the example you would have
   written anyway.
9. **Adoption is incremental.** Uncovered pages are reported, not failed. Coverage is
   per page.
10. **Nothing to forget.** No step between editing code and CI judging the docs that a
    human can skip. No source/output pairs, no generated files to commit.

## Scope

11. **Living and frozen are explicit.** The repo declares which docs are checked; frozen
    docs are excluded by name.

## Duality

12. **Code links back to docs.** For any public symbol, the tool lists the pages, lines
    and claim sentences that describe it, computed from the same parse that checks the
    docs, so the link cannot drift. Public symbols with no description are listed.

## Operations

13. **Blocking, fast, separate.** A job that can fail the pipeline, under a minute beyond
    the project's own build, independent of any site build.
14. **Deterministic and offline.** Same inputs, same verdict. Network only for snapshot
    refresh. No model calls.
15. **No cost to the shipped package.** Tooling dependencies only.

## Evidence

16. **Passes the corpus.** Catches every event in CORPUS.md at the commit that caused it
    and stays silent on the ordinary commits between.

## Out of scope

Rationale and architecture narrative. Prose quality. Anything that asks a model whether a
sentence is still true.
