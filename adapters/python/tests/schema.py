"""A small JSON Schema validator for the tests: the part of draft 2020-12 that the
protocol's schemas use (``type``, ``properties``, ``required``, ``additionalProperties``,
``items``, ``enum``, ``const``, ``anyOf``, ``minimum``, ``minLength`` and ``$ref`` within
and between files). The adapter itself needs no validator and has no dependencies.

The schemas are the package's own copy (``docattest_python/protocol/schema``), read once
when a ``Validator`` is made and never changed after.
"""

import json
import os

PACKAGE = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "docattest_python", "protocol"
)

_TYPES = {
    "object": lambda v: isinstance(v, dict),
    "array": lambda v: isinstance(v, list),
    "string": lambda v: isinstance(v, str),
    "boolean": lambda v: isinstance(v, bool),
    "null": lambda v: v is None,
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "integer": lambda v: (
        not isinstance(v, bool) and (isinstance(v, int) or (isinstance(v, float) and v.is_integer()))
    ),
}


class Validator:
    """The protocol's schema files, by name, and the checks made against them."""

    def __init__(self, files: dict) -> None:
        self.files = files

    @classmethod
    def load(cls, directory: str = os.path.join(PACKAGE, "schema")) -> "Validator":
        files = {}
        for name in sorted(os.listdir(directory)):
            if name.endswith(".json"):
                with open(os.path.join(directory, name), encoding="utf-8") as handle:
                    files[name] = json.load(handle)
        return cls(files)

    def _resolve(self, ref: str, file: str):
        name, _, pointer = ref.partition("#")
        file = name or file
        node = self.files[file]
        for part in filter(None, pointer.split("/")):
            node = node[part.replace("~1", "/").replace("~0", "~")]
        return node, file

    def problems(self, value, schema, file: str, at: str = "$") -> list:
        """Why ``value`` does not match ``schema``; empty when it does."""
        if "$ref" in schema:
            target, target_file = self._resolve(schema["$ref"], file)
            return self.problems(value, target, target_file, at)
        out = _literal_problems(value, schema, at)
        mismatch = _type_problem(value, schema, at)
        if mismatch:
            return out + [mismatch]
        out += self._any_of_problems(value, schema, file, at)
        out += _bound_problems(value, schema, at)
        if isinstance(value, dict):
            out += self._object_problems(value, schema, file, at)
        if isinstance(value, list) and "items" in schema:
            for k, item in enumerate(value):
                out += self.problems(item, schema["items"], file, f"{at}[{k}]")
        return out

    def _any_of_problems(self, value, schema, file: str, at: str) -> list:
        if "anyOf" not in schema:
            return []
        tried = [self.problems(value, option, file, at) for option in schema["anyOf"]]
        if all(tried):
            return [f"{at}: matches none of the alternatives ({'; '.join(min(tried, key=len))})"]
        return []

    def _object_problems(self, value: dict, schema, file: str, at: str) -> list:
        """``required``, ``properties`` and ``additionalProperties``."""
        properties = schema.get("properties", {})
        out = [f"{at}: lacks {key!r}" for key in schema.get("required", []) if key not in value]
        extra = schema.get("additionalProperties", True)
        for key, item in value.items():
            if key in properties:
                out += self.problems(item, properties[key], file, f"{at}.{key}")
            elif extra is False:
                out.append(f"{at}: has {key!r}, which the schema does not know")
            elif isinstance(extra, dict):
                out += self.problems(item, extra, file, f"{at}.{key}")
        return out

    def check(self, value, definition: str, file: str = "methods.json") -> list:
        """Problems of ``value`` against ``#/$defs/<definition>`` of a schema file."""
        return self.problems(value, self.files[file]["$defs"][definition], file)

    def check_exchange(self, requests: list, responses: list) -> list:
        """Every request and response of one conversation, against the schemas. A request the
        test sent malformed on purpose (the adapter answered it with an error) is not held to
        its params' shape."""
        out = []
        by_id = {r.get("id"): r for r in requests if isinstance(r, dict)}
        methods = self.files["methods.json"]["$defs"]
        for response in responses:
            out += [
                f"response {response.get('id')}: {p}"
                for p in self.check(response, "Response", "messages.json")
            ]
            request = by_id.get(response.get("id"))
            if (
                request is None
                or "result" not in response
                or f"{request.get('method')}.result" not in methods
            ):
                continue
            out += self._call_problems(request, response["result"])
        return out

    def _call_problems(self, request: dict, result) -> list:
        """A request that was answered with a result, and the result, against its method."""
        method = request["method"]
        out = [f"{method} request: {p}" for p in self.check(request, "Request", "messages.json")]
        out += [f"{method} params: {p}" for p in self.check(request.get("params", {}), f"{method}.params")]
        out += [f"{method} result: {p}" for p in self.check(result, f"{method}.result")]
        return out


def _literal_problems(value, schema, at: str) -> list:
    """``const`` and ``enum``."""
    out = []
    if "const" in schema and (value != schema["const"] or type(value) is not type(schema["const"])):
        out.append(f"{at}: is not {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        out.append(f"{at}: {value!r} is not one of {schema['enum']}")
    return out


def _type_problem(value, schema, at: str):
    """``type``: a problem, or None."""
    if "type" not in schema:
        return None
    allowed = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
    if any(_TYPES[t](value) for t in allowed):
        return None
    return f"{at}: {value!r} is not of type {'/'.join(allowed)}"


def _bound_problems(value, schema, at: str) -> list:
    """``minimum`` and ``minLength``."""
    out = []
    if (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and "minimum" in schema
        and value < schema["minimum"]
    ):
        out.append(f"{at}: {value} is below {schema['minimum']}")
    if isinstance(value, str) and len(value) < schema.get("minLength", 0):
        out.append(f"{at}: is shorter than {schema['minLength']}")
    return out
