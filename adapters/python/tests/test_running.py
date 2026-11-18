"""The protocol server running attested code: examples, sessions, cases, and the
workers they run in (isolation, time limits, the network, stdout).

    python3 -m unittest discover -s adapters/python/tests -v
"""

import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, SURFACE, Talk, example, fact, opened, run_cases, run_one, table  # noqa: E402

CASES_SETUP = "from lib import add, version\n\nasync def versions(row):\n    assert await version() == row['version']\n\ndef adds(row):\n    assert str(add(int(row['a']), int(row['b']))) == row['sum']\n\nadds.columns = ['a', 'b', 'sum']\ncases = {'adds': adds, 'versions': versions}\n"


class Running(unittest.TestCase):
    def test_a_failing_assertion_names_both_sides_and_the_block_line(self):
        result, _ = run_one("from lib import add\nassert add(2, 2) == 4")
        self.assertEqual(
            (result["status"], result["message"], result["page"], result["line"], result["kind"]),
            ("pass", "all assertions held", "docs/a.md", 3, "example"),
        )
        result, _ = run_one("from lib import add\n\nassert add(2, 2) == 5")
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["message"], "AssertionError: expected 5, got 4 (block line 3)")
        self.assertEqual((result["expected"], result["actual"]), ("5", "4"))
        result, _ = run_one("from lib import KEYS\nassert 'Z' in KEYS, 'Z should be a key'")
        self.assertEqual(result["message"], "AssertionError: Z should be a key (block line 2)")
        result, _ = run_one("from lib import add\nassert add(0, 0)")
        self.assertEqual(result["message"], "AssertionError: assert add(0, 0) (block line 2)")
        result, _ = run_one("def f():\n    raise ValueError('no\\nmore')\nf()")
        self.assertEqual(result["message"], "ValueError: no (block line 2)")

    def test_top_level_await_and_main(self):
        result, _ = run_one(
            "from lib import version\nassert await version() == '2.0'\nassert __name__ == '__main__'"
        )
        self.assertEqual(result["status"], "pass", result)

    def test_sessions_run_as_doctest_runs_them(self):
        good, _ = run_one(
            ">>> from lib import add\n>>> add(2, 2)\n4\n>>> add('a', 1)\nTraceback (most recent call last):\n    ...\nTypeError: can only concatenate str (not \"int\") to str",
            example={"lang": "pycon"},
        )
        self.assertEqual((good["status"], good["message"]), ("pass", "every output matched"))
        bad, _ = run_one(">>> from lib import add\n>>> add(2, 2)\n5", example={"lang": "python"})
        self.assertEqual(bad["status"], "fail")
        self.assertEqual(bad["message"], "`add(2, 2)`: expected 5, got 4 (block line 2)")
        self.assertEqual((bad["expected"], bad["actual"]), ("5", "4"))

    def test_a_block_tagged_continue_goes_on_from_the_blocks_before_it(self):
        files = {"lib.py": SURFACE, "docs/b.md": ""}
        go_on = {"tags": ["continue"]}
        page = [
            example("from lib import add\ntotal = add(1, 2)", id="a", line=3),
            example(
                ">>> total\n3\n>>> total = add(total, 1)\n>>> 1 / 0\nTraceback (most recent call last):\nZeroDivisionError: division by zero",
                id="b",
                line=9,
                lang="pycon",
                **go_on,
            ),
            example("assert total == 4\nassert add(total, 1) == 5", id="c", line=20, **go_on),
            # A block without the word starts again from nothing, and so does what follows it.
            example("assert 'total' not in globals()\nfresh = 1", id="d", line=30),
            example("assert fresh == 1 and 'total' not in globals()", id="e", line=40, **go_on),
            example("raise RuntimeError('no')", id="f", line=50),
            example("x = 1", id="g", line=60, **go_on),
        ]
        other = [
            example("assert True", id="first", line=2, page="docs/b.md", **go_on),
            example("print(total)", id="lost", line=8, page="docs/b.md"),
        ]
        every = page + other

        def run(k, e):
            return {"id": k, "method": "run", "params": {"examples": [e], "cases": []}}

        talk = Talk(
            HELLO,
            opened(files, examples=every),
            {"id": 3, "method": "compile", "params": {"examples": every}},
            {"id": 4, "method": "symbols", "params": {"attestations": every}},
            *[run(10 + k, e) for k, e in enumerate(page)],
        )
        # Names an earlier block of the session binds are bound; in a block that starts
        # anew they are not, and a first block has nothing to go on from.
        problems = {d["id"]: d["message"] for d in talk.result(3)}
        self.assertEqual(sorted(problems), ["first", "lost"], problems)
        self.assertIn("this page has none before this one", problems["first"])
        self.assertEqual(problems["lost"], "name 'total' is not defined")
        results = [talk.result(10 + k)[0] for k in range(len(page))]
        self.assertEqual(
            [r["status"] for r in results], ["pass", "pass", "pass", "pass", "pass", "fail", "fail"], results
        )
        self.assertEqual(results[6]["message"], "the block it continues (L50) did not run: RuntimeError: no")
        self.assertEqual([r["line"] for r in results], [3, 9, 20, 30, 40, 50, 60])
        # A block attests what it uses of the names its session imported.
        symbols = talk.result(4)
        self.assertEqual(symbols["c"], [{"name": "add", "surface": "default"}])
        self.assertEqual(symbols["d"], [])

    def test_a_docstring_session_starts_with_its_module(self):
        files = {
            "lib.py": 'def double(n):\n    """Doubles.\n\n    >>> double(2)\n    4\n    """\n    return n * 2\n'
        }
        e = example(">>> double(2)\n4", lang="pycon", page="lib.py", line=2, owner="double")
        talk = Talk(
            HELLO,
            opened(files),
            {"id": 3, "method": "run", "params": {"examples": [e], "cases": []}},
            {"id": 4, "method": "symbols", "params": {"attestations": [e]}},
        )
        self.assertEqual(talk.result(3)[0]["status"], "pass", talk.result(3))
        self.assertEqual(talk.result(4), {"e": [{"name": "double", "surface": "default"}]})

    def test_blocks_are_isolated_from_each_other(self):
        files = {"lib.py": "counter = {'n': 0}\n"}
        first = example(
            "import lib\nvalue = 1\nlib.counter['n'] += 1\nimport builtins\nbuiltins.leaked = True\nassert lib.counter['n'] == 1",
            id="a",
        )
        second = example(
            "import lib, builtins\nassert lib.counter['n'] == 0\nassert not hasattr(builtins, 'leaked')\nassert 'value' not in globals()",
            id="b",
            line=9,
        )
        talk = Talk(
            HELLO,
            opened(files, workers=1),
            {"id": 3, "method": "run", "params": {"examples": [first, second], "cases": []}},
        )
        self.assertEqual([r["status"] for r in talk.result(3)], ["pass", "pass"], talk.result(3))
        self.assertEqual([r["line"] for r in talk.result(3)], [3, 9])

    def test_a_stuck_block_is_stopped_and_the_next_one_runs(self):
        stuck = example("while True:\n    pass", id="a")
        asleep = example("import time\ntime.sleep(60)", id="b")
        fine = example("assert True", id="c")
        requests = [
            {"id": 3 + k, "method": "run", "params": {"examples": [e], "cases": []}}
            for k, e in enumerate([stuck, asleep, fine])
        ]
        talk = Talk(HELLO, opened({"lib.py": SURFACE}, workers=1, timeoutMs=300), *requests)
        self.assertEqual([talk.result(k)[0]["status"] for k in (3, 4, 5)], ["fail", "fail", "pass"])
        self.assertEqual(talk.result(3)[0]["message"], "timed out after 300 ms")

    def test_a_block_that_kills_its_process_fails_alone(self):
        dead, _ = run_one("import os\nos._exit(3)")
        self.assertEqual(
            (dead["status"], dead["message"]), ("fail", "the worker exited with code 3 before it finished")
        )

    def test_network_is_off_unless_a_block_asks(self):
        code = "import socket\nsocket.create_connection(('example.com', 80), timeout=2)"
        blocked, _ = run_one(code)
        self.assertEqual(blocked["status"], "fail")
        self.assertIn("network access is disabled", blocked["message"])
        raw, _ = run_one("import socket\ns = socket.socket()\ns.connect(('127.0.0.1', 9))")
        self.assertIn("network access is disabled", raw["message"])
        skipped, _ = run_one(code, example={"network": True})
        self.assertEqual(skipped["status"], "skipped")

    def test_an_error_that_escapes_on_a_thread_still_fails_the_block(self):
        code = "import threading\ndef work():\n    raise RuntimeError('lost')\nt = threading.Thread(target=work)\nt.start(); t.join()"
        result, _ = run_one(code)
        self.assertEqual(
            (result["status"], result["message"]),
            ("fail", "unhandled error from attested code: RuntimeError: lost"),
        )

    def test_cases_tables(self):
        files = {"lib.py": SURFACE, "docs/setup.py": CASES_SETUP}
        good = table(
            "adds",
            ["case", "a", "b", "sum"],
            [["small", "1", "2", "3"], ["wrong on purpose", "2", "2", "5"], ["bad", "x", "2", "5"]],
        )
        talk = Talk(
            HELLO,
            opened(files, setup="docs/setup.py"),
            run_cases(3, good),
            run_cases(4, table("nope", ["case"], [["a"]])),
            run_cases(5, table("adds", ["case", "a", "sum"], [["a", "1", "2"]])),
            {"id": 6, "method": "symbols", "params": {"attestations": [good]}},
            run_cases(7, table("versions", ["case", "version"], [["now", "2.0"], ["old", "1.0"]])),
        )
        self.assertEqual([r["status"] for r in talk.result(7)], ["pass", "fail"])
        rows = talk.result(3)
        self.assertEqual(
            [(r["line"], r["status"], r["kind"]) for r in rows],
            [(8, "pass", "cases"), (9, "fail", "cases"), (10, "fail", "cases")],
        )
        self.assertEqual(rows[0]["message"], "docs/t.md:8 small")
        self.assertEqual(
            rows[1]["message"], "docs/t.md:9 wrong on purpose: AssertionError: expected '5', got '4'"
        )
        self.assertEqual((rows[1]["expected"], rows[1]["actual"]), ("'5'", "'4'"))
        self.assertIn("ValueError", rows[2]["message"])
        self.assertEqual([(r["line"], r["status"]) for r in talk.result(4)], [(5, "error")])
        self.assertIn(
            'cases runner "nope" at L5 is not in `cases` of docs/setup.py (known: adds, versions)',
            talk.result(4)[0]["message"],
        )
        self.assertEqual(
            talk.result(5)[0]["message"],
            'cases "adds" at L5: the table is missing column "b" declared by the runner',
        )
        self.assertEqual(talk.result(6), {"adds": [{"name": "add", "surface": "default"}]})

    def test_stdout_carries_nothing_but_responses(self):
        surface = (
            "import os, sys\nprint('loading the surface')\nsys.stdout.write('raw surface output\\n')\nN = 3\n"
        )
        loud = example(
            "import os, subprocess, sys\nprint('hello from an example')\nsys.stdout.write('raw example output\\n')\nos.write(1, b'straight to descriptor 1\\n')\nsubprocess.run(['echo', 'from a child process'])\nsys.__stdout__.write('dunder stdout\\n'); sys.__stdout__.flush()"
        )
        talk = Talk(
            HELLO,
            opened({"lib.py": surface}),
            {"id": 3, "method": "evaluate", "params": {"facts": [fact("N")]}},
            {"id": 4, "method": "run", "params": {"examples": [loud], "cases": []}},
            {"id": 5, "method": "close", "params": {}},
        )
        self.assertEqual(talk.result(3)["f"]["value"]["text"], "3")
        self.assertEqual(talk.result(4)[0]["status"], "pass", talk.result(4))
        for line in talk.stdout.split("\n"):
            if line:
                self.assertIn("id", json.loads(line))
        for leaked in (
            "loading the surface",
            "raw surface",
            "raw example",
            "hello from",
            "straight to",
            "child process",
            "dunder",
        ):
            self.assertNotIn(leaked, talk.stdout)
        self.assertIn("straight to descriptor 1", talk.stderr)
        self.assertIn("from a child process", talk.stderr)

    def test_answers_arrive_as_requests_finish(self):
        slow = example("import time\ntime.sleep(0.4)")
        talk = Talk(
            HELLO,
            opened({"lib.py": SURFACE}),
            {"id": 3, "method": "run", "params": {"examples": [slow], "cases": []}},
            {"id": 4, "method": "exports", "params": {"surface": "default"}},
        )
        self.assertEqual([r["id"] for r in talk.responses][-2:], [4, 3])
        self.assertEqual(talk.status, 0)

    def test_without_fork_every_job_gets_its_own_process(self):
        first = example("import lib\nlib.KEYS.append('D')\nassert len(lib.KEYS) == 4", id="a")
        second = example("import lib\nassert len(lib.KEYS) == 3", id="b")
        talk = Talk(
            HELLO,
            opened({"lib.py": SURFACE}, workers=1),
            {"id": 3, "method": "run", "params": {"examples": [first, second], "cases": []}},
            {"id": 4, "method": "evaluate", "params": {"facts": [fact("N")]}},
            env={"DOCATTEST_PYTHON_NO_FORK": "1"},
        )
        self.assertEqual([r["status"] for r in talk.result(3)], ["pass", "pass"], talk.result(3))
        self.assertEqual(talk.result(4)["f"]["value"]["text"], "3")


if __name__ == "__main__":
    unittest.main()
