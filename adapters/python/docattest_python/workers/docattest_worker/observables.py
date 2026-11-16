"""The observables job: every value below an export that a page could have copied."""

from __future__ import annotations

import collections
import enum
from collections.abc import Iterable
from typing import Literal, NamedTuple, cast

from .channel import Channel
from .environment import Environment, surface_namespace
from .failures import failure_message
from .jobs import ObservablesJob
from .messages import Observable, Observation
from .named_facts import named_fact_values
from .values import describe_value, is_enum, observe, safe_str

# Bounds of the walk `observables` makes below one export (the TypeScript adapter's).
MAX_DEPTH = 6
MAX_NODES = 400
MAX_MEMBERS = 100
MAX_ITEMS = 500
MAX_PAIRED_FIELDS = 6

_SCALARS = (str, int, float, bool)

#: How a row's field is read: as an attribute (``r.price``) or as an item (``r["price"]``).
Access = Literal["attr", "item"]


class Found(NamedTuple):
    """A value the walk offers: the fact expression that reads it, and the keys on the way."""

    expression: str
    path: list[str]
    value: Observation


class Record(NamedTuple):
    """A row of a table a page could have copied: how its fields are read, and its scalar
    fields by name."""

    access: Access
    fields: dict[str, object]


def _scalar(value: object) -> bool:
    return isinstance(value, _SCALARS)


def _identifier(key: object) -> bool:
    return isinstance(key, str) and key.isidentifier()


def _writable(key: str) -> bool:
    return "-->" not in key and "\n" not in key and "\r" not in key


def _is_named_tuple(value: object) -> bool:
    return hasattr(value, "_fields") and isinstance(value, tuple)


def _record(value: object) -> Record | None:
    """A row as a record, or None when it is not one."""
    if isinstance(value, dict):
        items = cast("dict[object, object]", value).items()
        fields = {k: v for k, v in items if isinstance(k, str) and _scalar(v)}
        return Record("item", fields) if fields else None
    attributes = _attributes(value)
    if attributes is None:
        return None
    fields = {k: v for k, v in attributes.items() if _scalar(v)}
    return Record("attr", fields) if fields else None


def _attributes(value: object) -> dict[str, object] | None:
    """A dataclass instance's or a named tuple's fields by name, or None for anything else."""
    import dataclasses

    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return {f.name: getattr(value, f.name, None) for f in dataclasses.fields(value)}
    if _is_named_tuple(value):
        row = cast("tuple[object, ...]", value)
        names: tuple[str, ...] = getattr(value, "_fields")  # noqa: B009 - a named tuple's own
        return dict(zip(names, row))
    return None


def _read(access: Access, field: str, row: str = "r") -> str:
    return "%s.%s" % (row, field) if access == "attr" else "%s[%r]" % (row, field)


def _list_observable(expression: str, path: list[str], value: object, items: list[str]) -> Found:
    description = describe_value(value)
    text = safe_str(value)
    if isinstance(value, (set, frozenset)):
        text, items = _in_order(cast("set[object] | frozenset[object]", value))
    return Found(expression, path, {"text": text, "items": items, "description": description})


def _in_order(value: set[object] | frozenset[object]) -> tuple[str, list[str]]:
    """A set's text and items, sorted: a set of strings iterates in an order that changes from
    one process to the next, and what is offered must not."""
    ordered = sorted(value, key=safe_str)
    inner = "{%s}" % ", ".join(repr(item) for item in ordered) if ordered else ""
    text = "frozenset(%s)" % inner if isinstance(value, frozenset) else inner or "set()"
    return text, [safe_str(item) for item in ordered]


def _a_list(expression: str, path: list[str], shown: object, items: list[str]) -> Found:
    """An observable list the walk built itself: ``shown`` is what its text shows."""
    return Found(expression, path, {"text": safe_str(shown), "items": items, "description": "a list"})


def _fields(rows: list[object], expression: str, path: list[str], level: int) -> list[Found]:
    """Each field every row has, and at the top each ordered pair of fields."""
    if not rows or len(rows) > MAX_ITEMS or level > 1:
        return []
    records = _records(rows)
    if records is None:
        return []
    access = records[0].access
    names = _shared_fields(access, records)
    columns = _columns(records, names)
    out: list[Found] = []
    for f in names:
        expr = "[%s for r in %s]" % (_read(access, f), expression)
        out.append(_a_list(expr, path + [f], columns[f], columns[f]))
    names = [f for f in names if _pairable(f)]
    if level > 0 or len(names) > MAX_PAIRED_FIELDS:
        return out
    out.extend(_pairs(access, names, columns, expression, path))
    return out


def _records(rows: list[object]) -> list[Record] | None:
    """The rows as records, or None when one is not a record or they are not read alike."""
    records = [_record(r) for r in rows]
    found = [r for r in records if r is not None]
    if len(found) != len(records) or len({r.access for r in found}) != 1:
        return None
    return found


def _shared_fields(access: Access, records: list[Record]) -> list[str]:
    """The fields every record has, which a fact expression can read."""
    names = [f for f in records[0].fields if _writable(f) and all(f in r.fields for r in records)]
    if access == "attr":
        names = [f for f in names if _identifier(f)]
    return names


def _columns(records: list[Record], names: list[str]) -> dict[str, list[str]]:
    """Each field's values, one per row, as text."""
    return {f: [safe_str(r.fields[f]) for r in records] for f in names}


def _pairable(field: str) -> bool:
    # A pair is written inside an f-string, which a quote or a backslash in a key would end.
    return '"' not in field and "'" not in field and "\\" not in field


def _pairs(
    access: Access, names: list[str], columns: dict[str, list[str]], expression: str, path: list[str]
) -> list[Found]:
    """Each ordered pair of two fields, read together as one string per row."""
    out: list[Found] = []
    for a in names:
        for b in names:
            if a == b:
                continue
            expr = '[f"{%s} {%s}" for r in %s]' % (
                _read(access, a).replace('"', "'"),
                _read(access, b).replace('"', "'"),
                expression,
            )
            items = ["%s %s" % (x, y) for x, y in zip(columns[a], columns[b])]
            out.append(_a_list(expr, path + [a, b], items, items))
    return out


#: A member the walk descends into: its key, how it is read (``.name``, ``["key"]``), its value.
Member = tuple[str, str, object]


def _members(value: object) -> list[Member] | None:
    """The members the walk descends into, or None."""
    import dataclasses
    import enum
    import types

    if isinstance(value, dict):
        items = list(cast("dict[object, object]", value).items())[:MAX_MEMBERS]
        return [(k, "[%r]" % k, v) for k, v in items if isinstance(k, str)]
    if isinstance(value, types.ModuleType):
        return _module_members(value)
    if isinstance(value, type) and not issubclass(value, enum.Enum):
        return _class_members(cast("type[object]", value))
    if dataclasses.is_dataclass(value) or _is_named_tuple(value):
        return _record_members(value)
    return None


def _module_members(module: object) -> list[Member]:
    import types

    names: list[str] = getattr(module, "__all__", None) or [n for n in vars(module) if not n.startswith("_")]
    return [
        (n, "." + n, getattr(module, n))
        for n in list(names)[:MAX_MEMBERS]
        if hasattr(module, n) and not isinstance(getattr(module, n), types.ModuleType)
    ]


def _class_members(cls: type) -> list[Member]:
    out: list[Member] = []
    for n, v in list(vars(cls).items())[:MAX_MEMBERS]:
        if not n.startswith("_") and not callable(v) and not hasattr(v, "__get__"):
            out.append((n, "." + n, v))
    return out


def _record_members(value: object) -> list[Member]:
    import dataclasses

    if dataclasses.is_dataclass(value):
        names = [f.name for f in dataclasses.fields(value)]
    else:
        names = list(getattr(value, "_fields"))  # noqa: B009 - a named tuple's own
    return [(n, "." + n, getattr(value, n)) for n in names[:MAX_MEMBERS] if hasattr(value, n)]


def observables_of(name: str, root: object) -> list[Found]:
    """Every value below one export that a page could have copied, each with the fact
    expression that reads it, shallowest first."""
    return _Walk(name, root).run()


class _Node(NamedTuple):
    """A value the walk reaches: the fact expression that reads it, its path of keys, how
    many rows of records it is below (``level``) and how many steps below the export
    (``depth``)."""

    value: object
    expression: str
    path: list[str]
    level: int
    depth: int


class _Walk:
    """The walk below one export, breadth first: what it found, the values it has been
    through, and the nodes still to visit."""

    def __init__(self, name: str, root: object) -> None:
        self.out: list[Found] = []
        self.seen: set[int] = set()
        self.queue = collections.deque([_Node(root, name, [name], 0, 0)])

    def run(self) -> list[Found]:
        count = 0
        while self.queue and count < MAX_NODES:
            node = self.queue.popleft()
            if self._seen_before(node.value):
                continue
            count += 1
            try:
                self._visit(node)
            except BaseException:  # noqa: BLE001 - a value that cannot be read is not offered
                continue
        return self.out

    def _seen_before(self, value: object) -> bool:
        """Whether a value other than a scalar was visited already; marks it visited."""
        if isinstance(value, _SCALARS) or value is None:
            return False
        if id(value) in self.seen:
            return True
        self.seen.add(id(value))
        return False

    def _visit(self, node: _Node) -> None:
        """What one value offers. What it adds stays even when reading it fails halfway."""
        value = node.value
        if _scalar(value):
            self.out.append(Found(node.expression, node.path, observe(value)))
        elif is_enum(value):
            self._enum(node)
        elif not hasattr(value, "_fields") and isinstance(value, (list, tuple, set, frozenset)):
            self._collection(node)
        else:
            self._container(node)

    def _enum(self, node: _Node) -> None:
        members: list[enum.Enum] = list(cast("type[enum.Enum]", node.value))
        if len(members) > MAX_ITEMS:
            return
        names = [m.name for m in members]
        self.out.append(_a_list("[m.name for m in %s]" % node.expression, node.path, names, names))
        if all(_scalar(m.value) for m in members):
            values = [safe_str(m.value) for m in members]
            shown = [m.value for m in members]
            self.out.append(_a_list("[m.value for m in %s]" % node.expression, node.path, shown, values))

    def _collection(self, node: _Node) -> None:
        value = node.value
        items = list(cast("Iterable[object]", value))
        if len(items) <= MAX_ITEMS and all(_scalar(i) for i in items):
            self.out.append(_list_observable(node.expression, node.path, value, [safe_str(i) for i in items]))
        elif isinstance(value, (list, tuple)):
            self.out.extend(_fields(items, node.expression, node.path, node.level))

    def _container(self, node: _Node) -> None:
        """A dict, a module, a class or a record: what it offers, then its members."""
        import types

        value = node.value
        members = _members(value)
        if members is None:
            return
        namespace = isinstance(value, (types.ModuleType, type))
        if isinstance(value, dict):
            self._dict(node, cast("dict[object, object]", value))
        if node.depth >= MAX_DEPTH:
            return
        below = node.level if namespace else node.level + 1
        for key, access, member in members:
            if _writable(key):
                self.queue.append(
                    _Node(member, node.expression + access, node.path + [key], below, node.depth + 1)
                )

    def _dict(self, node: _Node, value: dict[object, object]) -> None:
        expression, path = node.expression, node.path
        keys = [k for k in value if isinstance(k, str)]
        if len(keys) > MAX_ITEMS or len(keys) != len(value):
            return
        self.out.append(_a_list("list(%s)" % expression, path, keys, keys))
        values = list(value.values())
        if values and all(_scalar(v) for v in values):
            texts = [safe_str(v) for v in values]
            self.out.append(_a_list("list(%s.values())" % expression, path, values, texts))
        self.out.extend(_fields(values, "%s.values()" % expression, path, node.level))


def run_observables(job: ObservablesJob, env: Environment, channel: Channel) -> None:
    surface = job["surface"]
    env.enter(False)
    try:
        namespace = surface_namespace(surface)
    except BaseException as err:  # noqa: BLE001
        message = 'surface "%s" failed to load: %s' % (surface["name"], failure_message(err))
        channel.post({"t": "failed", "message": message, "internal": None})
        return
    channel.post({"t": "ready"})
    names = surface["exports"] or sorted(namespace)
    roots = [(name, namespace[name]) for name in names if name in namespace and name.isidentifier()]
    if job["setup"] is not None:
        roots.extend(named_fact_values(env, job["setup"], namespace))
    found: list[Observable] = [
        {"expression": f.expression, "path": f.path, "value": f.value, "surface": surface["name"]}
        for name, value in roots
        for f in observables_of(name, value)
    ]
    channel.post({"t": "observables", "observables": found})
