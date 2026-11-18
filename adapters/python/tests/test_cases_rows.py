"""How the rows of cases tables run: one after another in one process per table, what a
row leaves behind for the next, and what a row that raises, exits or overruns does to the
rows after it.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from talk import HELLO, SURFACE, Talk, example, opened, table  # noqa: E402

# Each row first checks which rows ran before it in its process, then does what it says.
ROWS_SETUP = """\
import os, signal, sys, threading, time

_SEEN = []


def _on_signal(signum, frame):
    pass


def _left_behind():
    return any(t.name == "left behind" for t in threading.enumerate())


def rows(row):
    before = " ".join(_SEEN)
    _SEEN.append(row["row"])
    assert before == row["after"]
    do = row["do"]
    if do == "start a thread":
        threading.Thread(target=time.sleep, args=(30,), name="left behind", daemon=True).start()
    elif do == "handle a signal":
        signal.signal(signal.SIGUSR1, _on_signal)
    elif do == "look":
        assert _left_behind() and signal.getsignal(signal.SIGUSR1) is _on_signal
    elif do == "look for none":
        assert not _left_behind() and signal.getsignal(signal.SIGUSR1) is signal.SIG_DFL
    elif do == "raise":
        raise RuntimeError("on purpose")
    elif do == "exit":
        sys.exit(3)
    elif do == "end the process":
        os._exit(4)
    elif do == "hang":
        time.sleep(60)


def meet(row):
    here = os.path.dirname(__file__)
    open(os.path.join(here, row["arrive"]), "w").close()
    others = [os.path.join(here, name) for name in row["wait for"].split()]
    for _ in range(2000):
        if all(os.path.exists(other) for other in others):
            return
        time.sleep(0.01)
    raise AssertionError("the others never ran")


cases = {"rows": rows, "meet": meet}
"""

HEADER = ["row", "do", "after"]


def rows_of(*rows):
    return table("rows", HEADER, [list(r) for r in rows])


# A block that meets the tables as `meet` does: it arrives, then waits for the others.
MEETING_BLOCK = """\
import os, time
open(os.path.join("docs", "c"), "w").close()
others = [os.path.join("docs", name) for name in ("a", "b")]
for _ in range(2000):
    if all(os.path.exists(other) for other in others):
        break
    time.sleep(0.01)
assert all(os.path.exists(other) for other in others)
"""


def run_tables(*tables, examples=(), **settings):
    files = {"lib.py": SURFACE, "docs/setup.py": ROWS_SETUP}
    run = {"id": 3, "method": "run", "params": {"examples": list(examples), "cases": list(tables)}}
    return Talk(HELLO, opened(files, setup="docs/setup.py", **settings), run).result(3)


class CasesRows(unittest.TestCase):
    def test_a_tables_rows_share_its_process(self):
        results = run_tables(
            rows_of(
                ("a", "", ""),
                ("b", "start a thread", "a"),
                ("c", "handle a signal", "a b"),
                ("d", "raise", "a b c"),
                ("e", "look", "a b c d"),
                ("f", "exit", "a b c d e"),
                ("g", "look", "a b c d e f"),
                ("h", "end the process", "a b c d e f g"),
                ("i", "look for none", ""),
                ("j", "hang", "i"),
                ("k", "look for none", ""),
            ),
            timeoutMs=2000,
        )
        self.assertEqual(
            [(r["status"], r["message"]) for r in results],
            [
                ("pass", "docs/t.md:8 a"),
                ("pass", "docs/t.md:9 b"),
                ("pass", "docs/t.md:10 c"),
                ("fail", "docs/t.md:11 d: RuntimeError: on purpose"),
                ("pass", "docs/t.md:12 e"),
                ("fail", "docs/t.md:13 f: SystemExit: 3"),
                ("pass", "docs/t.md:14 g"),
                ("fail", "docs/t.md:15 h: the worker exited with code 4 before it finished"),
                ("pass", "docs/t.md:16 i"),
                ("fail", "docs/t.md:17 j: timed out after 2000 ms"),
                ("pass", "docs/t.md:18 k"),
            ],
        )

    def test_tables_run_side_by_side_and_never_share_a_process(self):
        # Each waits until the other two have arrived, so they pass only when all three run
        # at once.
        first = table("meet", ["row", "arrive", "wait for"], [["first", "a", "b c"]])
        second = table("meet", ["row", "arrive", "wait for"], [["second", "b", "a c"]])
        threaded = rows_of(("w", "start a thread", ""))
        fresh = rows_of(("x", "look for none", ""))
        results = run_tables(
            first, second, threaded, fresh, examples=[example(MEETING_BLOCK)], workers=3, timeoutMs=30000
        )
        self.assertEqual([r["status"] for r in results], ["pass"] * 5, results)


if __name__ == "__main__":
    unittest.main()
