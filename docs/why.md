---
title: Why
---

# Why

Rationale for the design, kept apart from the pages that make claims so that it can
stay general and therefore stay true. This page is expected to appear as uncovered.

## Why execution and not change detection

A test body changing is not evidence that documented behaviour changed. Tests change
for refactors, fixtures, renames and tightening far more often than behaviour does. On
one real history a change-based check fired hundreds of times and the documentation
moved zero times afterwards. Only running the claim distinguishes a refactor from a
change, so the claim has to be runnable.

## Why the expectation lives in the page

Developers fix where the failure is. If the failure is in a test file, the doc two
directories away never gets opened. If the failure names the page and the line, the
page gets opened. Colocation is the whole mechanism.

## Why facts only observe the surface

A page is read by users of the surface. If a fact needs an internal constant, the
sentence is either about something users cannot see, which does not belong on the page,
or about something that should be exported. Either way the constraint improves the
page.

## Why nothing is generated

Generated files need a step that can be forgotten and a diff that nobody reads. A fact
that compares and fails forces a human or an agent to edit the prose, which is the
behaviour we want and the only one that keeps the prose a document rather than an
output.
