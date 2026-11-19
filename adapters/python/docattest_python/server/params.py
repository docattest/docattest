"""Typed reads of request params: where the core's JSON becomes the adapter's types. A
missing or mistyped field is the core's mistake, reported as ``internal``."""

from __future__ import annotations

import os

from ..analysis.surfaces import ConfiguredSurface, SurfaceAlias
from ..attestations import Attestation, Cases, CasesRow, Example, Fact, TestCitation
from ..language import Snippet, SourceText
from ..session.mutants import MutatedSymbol
from ..session.settings import SessionSettings
from ..test_runs.outcome import RequestedTest
from ..workers.docattest_worker.fields import Fields, MalformedMessage


class Params(Fields):
    """The params of one request; messages name the method."""

    def __init__(self, method: str, params: dict[str, object]) -> None:
        super().__init__(params, method)

    def sources(self, name: str) -> list[SourceText]:
        return [SourceText(f.string("page"), f.string("text")) for f in self.objects(name)]

    def snippets(self, name: str) -> list[Snippet]:
        return [Snippet(f.string("lang"), f.string("code")) for f in self.objects(name)]

    def examples(self, name: str) -> list[Example]:
        return [_read_example(f) for f in self.objects(name)]

    def facts(self, name: str) -> list[Fact]:
        return [_read_fact(f) for f in self.objects(name)]

    def cases(self, name: str) -> list[Cases]:
        return [_read_cases(f) for f in self.objects(name)]

    def attestations(self, name: str) -> list[Attestation]:
        return [_read_attestation(f) for f in self.objects(name)]

    def requested_tests(self, name: str) -> list[RequestedTest]:
        """``runTests``' tests, each as its file declares it."""
        return [
            RequestedTest(f.string("file"), tuple(f.strings("describes")), f.string("title"))
            for f in self.objects(name)
        ]

    def mutated_symbols(self, name: str) -> list[MutatedSymbol]:
        """``mutants``' symbols, each as ``symbols`` answered it."""
        return [MutatedSymbol(f.string("name"), f.optional_string("surface")) for f in self.objects(name)]

    def base(self, name: str) -> dict[str, str | None]:
        """``affected``'s earlier texts: a map of strings, null for a file that was not there."""
        base = self.json_object(name)
        texts = {k: v for k, v in base.items() if v is None or isinstance(v, str)}
        if len(texts) != len(base):
            raise MalformedMessage(f'{self.where}: "{name}" must be a map of strings')
        return texts

    def session_settings(self) -> SessionSettings:
        """``open``'s params."""
        snapshots = self.object("snapshots")
        return SessionSettings(
            root=os.path.abspath(self.string("root")),
            files=self.string_map("files") if self.has("files") else None,
            surfaces=[_read_surface(f) for f in self.objects("surfaces")],
            setup=self.optional_string("setup"),
            timeout_ms=self.number("timeoutMs"),
            workers=int(self.number("workers")),
            network=self.boolean("network"),
            snapshots={"dir": snapshots.optional_string("dir"), "data": snapshots.json_object("data")},
            manifests=self.json_object("manifests"),
            texts=self.string_map("texts") if self.has("texts") else {},
            matches=self.string_maps("matches") if self.has("matches") else {},
            manifest_fields=self.json_objects("manifestFields") if self.has("manifestFields") else {},
            examples=self.examples("examples"),
        )


def _read_example(f: Fields) -> Example:
    return Example(
        id=f.string("id"),
        page=f.string("page"),
        line=f.integer("line"),
        lang=f.string("lang"),
        code=f.string("code"),
        network=f.boolean("network"),
        weak=f.boolean("weak"),
        owner=f.optional_string("owner"),
        tags=tuple(f.strings("tags")) if f.has("tags") else (),
        error=f.optional_string("error"),
    )


def _read_fact(f: Fields) -> Fact:
    return Fact(
        id=f.string("id"),
        page=f.string("page"),
        line=f.integer("line"),
        expression=f.string("expression"),
        surface=f.string("surface"),
        error=f.optional_string("error"),
    )


def _read_cases(f: Fields) -> Cases:
    rows = tuple(
        CasesRow(r.integer("line"), r.string("name"), r.string_map("values")) for r in f.objects("rows")
    )
    return Cases(
        id=f.string("id"),
        page=f.string("page"),
        line=f.integer("line"),
        runner=f.string("runner"),
        header=tuple(f.strings("header")),
        rows=rows,
        error=f.optional_string("error"),
    )


def _read_test(f: Fields) -> TestCitation:
    return TestCitation(
        id=f.string("id"),
        page=f.string("page"),
        line=f.integer("line"),
        file=f.string("file"),
        title=f.string("title"),
        describes=tuple(f.strings("describes")),
        error=f.optional_string("error"),
    )


def _read_attestation(f: Fields) -> Attestation:
    kind = f.string("kind")
    if kind == "example":
        return _read_example(f)
    if kind == "fact":
        return _read_fact(f)
    if kind == "cases":
        return _read_cases(f)
    if kind == "test":
        return _read_test(f)
    raise MalformedMessage(f'{f.where}: unknown kind "{kind}"')


def _read_surface(f: Fields) -> ConfiguredSurface:
    if f.holds_object("entry"):
        alias = f.object("entry")
        return ConfiguredSurface(f.string("name"), SurfaceAlias(alias.string("import"), alias.string("path")))
    return ConfiguredSurface(f.string("name"), f.string("entry"))
