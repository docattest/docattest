"""What the protocol tests share: ``Talk`` drives the server the way the core drives it
(request lines in, response lines out, one process per conversation), and builds the
requests they send."""

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

import schema  # noqa: E402

#: The protocol's schemas, read once; every conversation is held to them.
SCHEMAS = schema.Validator.load()


class Talk:
    def __init__(self, *lines, env=None):
        text = "".join((line if isinstance(line, str) else json.dumps(line)) + "\n" for line in lines)
        environment = dict(os.environ, PYTHONPATH=PACKAGE_ROOT)
        environment.update(env or {})
        done = subprocess.run(
            [sys.executable, "-m", "docattest_python"],
            input=text.encode(),
            capture_output=True,
            env=environment,
            timeout=120,
        )
        self.status = done.returncode
        self.stdout = done.stdout.decode()
        self.stderr = done.stderr.decode()
        self.responses = [json.loads(line) for line in self.stdout.split("\n") if line]
        # Every conversation in these tests is held to the published schemas, so that the
        # adapter and the schemas cannot drift apart.
        requests = [line for line in lines if isinstance(line, dict)]
        drift = SCHEMAS.check_exchange(requests, self.responses)
        assert not drift, "\n".join(drift)

    def of(self, rid):
        found = [r for r in self.responses if r["id"] == rid]
        assert len(found) == 1, (rid, self.responses, self.stderr)
        return found[0]

    def result(self, rid):
        response = self.of(rid)
        assert "result" in response, (response, self.stderr)
        return response["result"]

    def error(self, rid):
        return self.of(rid)["error"]


HELLO = {"id": 1, "method": "hello", "params": {"protocol": 1}}
SURFACE = """\
N = 3
RATE = 1.5
ON = True
NOTHING = None
KEYS = ["A", "B", "C"]
PAIR = ("x", 1)
TAGS = {"only"}
LIMITS = {"small": 1, "large": 9}
SHAPE = "class Point:\\n    x: int\\n    y: int"
_PRIVATE = 7


def add(a, b):
    return a + b


async def version():
    return "2.0"
"""


def opened(files, **extra):
    params = {
        "root": "/not/read",
        "files": files,
        "surfaces": [{"name": "default", "entry": "./lib.py"}],
        "timeoutMs": 5000,
        "workers": 2,
        "network": False,
        "snapshots": {"data": {}},
        "manifests": {},
        "examples": [],
    }
    params.update(extra)
    return {"id": 2, "method": "open", "params": params}


def example(code, **fields):
    """An example as the core sends it: ``fields`` (``id``, ``mode``, ``lang``, ``line``,
    anything else) replace or add to the defaults."""
    base = {
        "id": "e",
        "kind": "example",
        "page": "docs/a.md",
        "line": 3,
        "claim": "",
        "lang": "python",
        "mode": "check",
        "network": False,
        "weak": False,
        "code": code,
    }
    base.update(fields)
    return base


def fact(expression, id="f", surface="default"):
    return {
        "id": id,
        "kind": "fact",
        "page": "docs/a.md",
        "line": 1,
        "claim": "",
        "expression": expression,
        "surface": surface,
        "ordered": False,
    }


def run_one(code, files=None, **extra):
    e = example(code, **extra.pop("example", {}))
    talk = Talk(
        HELLO,
        opened(files or {"lib.py": SURFACE}, **extra),
        {"id": 3, "method": "run", "params": {"examples": [e], "cases": []}},
    )
    return talk.result(3)[0], talk
