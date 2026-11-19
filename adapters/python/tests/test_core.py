"""The fixture project, checked by the real docattest core driving this adapter.

Skipped unless a core binary is found: ``DOCATTEST_CORE``, else
``core/target/release/docattest`` of the repository this adapter lives in.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
FIXTURE = os.path.join(HERE, "fixture")
CORE = os.environ.get("DOCATTEST_CORE") or os.path.join(
    PACKAGE_ROOT, "..", "..", "core", "target", "release", "docattest"
)
USABLE = os.path.isfile(CORE)

# docattest as a Python project runs it: through this package's launcher, which finds the
# core and tells it how to start the adapter. Nothing here says where the adapter is.
ENV = dict(
    os.environ,
    PYTHONPATH=PACKAGE_ROOT,
    DOCATTEST_CORE=os.path.abspath(CORE),
    GIT_CONFIG_GLOBAL=os.devnull,
    GIT_CONFIG_SYSTEM=os.devnull,
    GIT_AUTHOR_NAME="t",
    GIT_AUTHOR_EMAIL="t@example.invalid",
    GIT_COMMITTER_NAME="t",
    GIT_COMMITTER_EMAIL="t@example.invalid",
)
ENV.pop("DOCATTEST_NETWORK", None)
ENV.pop("DOCATTEST_ADAPTER_PYTHON", None)
ENV.pop("DOCATTEST_COMMAND", None)


def core(*args, root=FIXTURE, stdin=""):
    done = subprocess.run(
        [sys.executable, "-m", "docattest", *args, "--root", root],
        input=stdin,
        capture_output=True,
        text=True,
        env=ENV,
        timeout=120,
    )
    return done.returncode, done.stdout, done.stderr


def statuses(report, page):
    return [(r["line"], r["kind"], r["status"]) for r in report["results"] if r["page"] == page]


# What `check` says of each attestation of the fixture, page by page: (line, kind, status).
STATUSES = {
    "docs/pricing.md": [
        (5, "example", "pass"),
        (14, "example", "pass"),
        (22, "example", "pass"),
        (31, "example", "pass"),
        (36, "fact", "pass"),
        (37, "fact", "pass"),
        (39, "fact", "pass"),
        (44, "fact", "pass"),
        (51, "fact", "pass"),
        (56, "fact", "pass"),
        (64, "cases", "pass"),
        (65, "cases", "pass"),
        (66, "cases", "pass"),
        (68, "test", "pass"),
    ],
    "docs/shipping.md": [
        (3, "fact", "pass"),
        (10, "example", "pass"),
        (19, "example", "skipped"),
        (27, "cases", "pass"),
        (28, "cases", "pass"),
    ],
    "src/shoplib/pricing.py": [(20, "example", "pass"), (40, "example", "pass"), (55, "example", "pass")],
    "docs/broken.md": [
        (5, "example", "fail"),
        (11, "example", "fail"),
        (17, "example", "fail"),
        (21, "example", "fail"),
        (26, "example", "fail"),
        (31, "example", "fail"),
        (36, "example", "error"),
        (40, "fact", "fail"),
        (42, "fact", "fail"),
        (46, "fact", "error"),
        (51, "cases", "fail"),
        (52, "cases", "fail"),
        (54, "cases", "error"),
        (59, "test", "error"),
    ],
}


@unittest.skipUnless(USABLE, "no docattest core binary (set DOCATTEST_CORE)")
class Fixture(unittest.TestCase):
    maxDiff = None

    def test_check_passes_the_true_pages_and_fails_the_false_one(self):
        code, out, err = core("check", "--json")
        self.assertEqual(code, 1, err)
        report = json.loads(out)
        self.assertEqual(report["exitCode"], 1)
        for page, expected in STATUSES.items():
            self.assertEqual(statuses(report, page), expected, page)
        self.assert_broken_page_says_why(report)
        self.assert_coverage_and_surfaces(report)

    def assert_broken_page_says_why(self, report):
        broken = {r["line"]: r for r in report["results"] if r["page"] == "docs/broken.md"}
        self.assertEqual(
            broken[5]["message"],
            "AssertionError: expected Money(cents=700, currency='EUR'), got Money(cents=750, currency='EUR') (block line 3)",
        )
        self.assertEqual(
            (broken[5]["expected"], broken[5]["actual"]),
            ("Money(cents=700, currency='EUR')", "Money(cents=750, currency='EUR')"),
        )
        self.assertEqual(
            broken[17]["message"],
            "cannot import name 'aply_discount' from 'shoplib' (src/shoplib/__init__.py) (block line 1)",
        )
        self.assertEqual(broken[26]["message"], "timed out after 3000 ms")
        self.assertIn("network access is disabled", broken[31]["message"])
        self.assertEqual((broken[42]["expected"], broken[42]["actual"]), ("[NL, BE]", "[NL, DE, FR]"))
        self.assertEqual(broken[5]["symbols"], ["Money", "apply_discount"])

    def assert_coverage_and_surfaces(self, report):
        coverage = report["coverage"]
        self.assertEqual(
            (coverage["living"], coverage["covered"], coverage["uncovered"], coverage["cited"]),
            (5, 4, ["docs/notes.md"], 2),
        )
        self.assertEqual(
            report["surfaces"]["default"]["resolution"],
            {
                "import": "shoplib",
                "entry": "src/shoplib/__init__.py",
                "package": {"name": "shoplib", "version": "1.2.3", "display": "shoplib 1.2.3"},
            },
        )
        # A fact over CARRIERS attests Carrier, which CARRIERS is computed from.
        self.assertEqual(report["surfaces"]["shipping"]["unattested"], ["ROUTES", "fetch_rates", "quote"])

    def test_the_true_pages_alone_exit_zero(self):
        code, out, err = core("check", "docs/pricing.md", "docs/shipping.md", "src/shoplib/pricing.py")
        self.assertEqual(code, 0, out + err)
        self.assertIn("results: 21 pass, 0 fail, 0 error, 1 skipped", out)

    def test_the_index_goes_from_symbols_to_pages(self):
        code, out, _ = core("index", "--json")
        index = json.loads(out)
        self.assertEqual(code, 0)
        self.assertIn(
            {
                "page": "docs/pricing.md",
                "line": 36,
                "kind": "fact",
                "claim": "Prices are in `EUR` unless told otherwise.",
            },
            index["DEFAULT_CURRENCY"],
        )
        self.assertIn(
            ("src/shoplib/pricing.py", 40, "example"),
            [(r["page"], r["line"], r["kind"]) for r in index["apply_discount"]],
        )
        code, out, _ = core("where", "fetch_rates")
        self.assertEqual(
            (code, out), (0, "docs/shipping.md:10 example  A quote can be awaited from the carrier.\n")
        )
        code, out, _ = core("uncovered")
        self.assertEqual((code, out.split()), (0, ["CARRIERS", "Carrier", "ROUTES"]))

    def test_audit_and_inventory_read_the_surface_values(self):
        code, out, err = core("audit", "--json")
        self.assertEqual(code, 0, err)
        found = {
            (s["page"], s["line"], s["target"], s["status"], s["expression"])
            for s in json.loads(out)["suggestions"]
        }
        self.assertIn(("docs/notes.md", 10, "column", "drift", "list(ERROR_CODES)"), found)
        code, out, _ = core("inventory", "EUR")
        self.assertEqual(code, 0)
        self.assertIn("DEFAULT_CURRENCY", out)


@unittest.skipUnless(USABLE and shutil.which("git"), "needs the docattest core and git")
class Changes(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="docattest-python-")
        self.root = os.path.join(os.path.realpath(self.temp.name), "project")
        shutil.copytree(FIXTURE, self.root, ignore=shutil.ignore_patterns("__pycache__", "broken.md"))
        for args in (["init", "-q", "-b", "main"], ["add", "-A"], ["commit", "-q", "-m", "base"]):
            subprocess.run(["git", *args], cwd=self.root, env=ENV, check=True, capture_output=True)

    def tearDown(self):
        self.temp.cleanup()

    def edit(self, rel, old, new):
        path = os.path.join(self.root, rel)
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn(old, text)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text.replace(old, new))

    def test_impact_follows_a_change_to_the_pages_that_name_it(self):
        self.edit("src/shoplib/shipping.py", 'return route["days"]', 'return int(route["days"])')
        self.edit("src/shoplib/pricing.py", "return int(value + 0.5)", "return int(value + 0.5)  # half up")
        code, out, err = core("impact", "--json", root=self.root)
        self.assertEqual(code, 0, err)
        impact = json.loads(out)
        self.assertEqual(impact["symbols"], ["fetch_rates", "quote"])
        self.assertIn(
            ("quote", "docs/shipping.md", "cases"),
            [(r["symbol"], r["page"], r["kind"]) for r in impact["references"]],
        )
        event = json.dumps({"tool_input": {"file_path": os.path.join(self.root, "src/shoplib/shipping.py")}})
        code, out, _ = core("impact", "--hook", root=self.root, stdin=event)
        self.assertIn(
            "this edit affects 2 symbols",
            json.loads(out)["hookSpecificOutput"]["additionalContext"],
        )
        quiet = json.dumps({"tool_input": {"file_path": os.path.join(self.root, "docs/notes.md")}})
        self.assertEqual(core("impact", "--hook", root=self.root, stdin=quiet)[:2], (0, ""))

    def test_any_agent_or_script_can_ask_with_plain_commands(self):
        self.edit("src/shoplib/shipping.py", 'return route["days"]', 'return int(route["days"])')

        def run(*args, cwd):
            return subprocess.run(
                [sys.executable, "-m", "docattest", *args],
                capture_output=True,
                text=True,
                env=ENV,
                cwd=cwd,
                timeout=120,
            )

        # From below the root, with no --root and no agent's event: the root is found.
        done = run("impact", "--file", "shipping.py", cwd=os.path.join(self.root, "src", "shoplib"))
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("changes since HEAD affect 2 symbols", done.stdout)
        done = run("impact", "--file", os.path.join(self.root, "docs", "notes.md"), cwd=self.root)
        self.assertEqual((done.returncode, done.stdout), (0, ""), done.stderr)
        # The baseline for an agent with no hooks: a section of AGENTS.md, and a git hook.
        with open(os.path.join(self.root, "AGENTS.md"), "w", encoding="utf-8") as handle:
            handle.write("# Mine\n")
        done = run("agents", "install", cwd=self.root)
        self.assertEqual(done.returncode, 0, done.stderr)
        with open(os.path.join(self.root, "AGENTS.md"), encoding="utf-8") as handle:
            written = handle.read()
        self.assertTrue(written.startswith("# Mine\n\n<!-- docattest:begin -->\n"), written)
        self.assertRegex(written, r"run `[^`\n]*docattest impact --file <path>`")
        self.assertNotIn("npx", written)
        done = run("hooks", "install", "--git", cwd=self.root)
        self.assertEqual(done.returncode, 0, done.stderr)
        hook = os.path.join(self.root, ".git", "hooks", "pre-commit")
        self.assertTrue(os.access(hook, os.X_OK))
        with open(hook, encoding="utf-8") as handle:
            self.assertRegex(handle.read(), r"docattest check --quiet \|\| exit 1\n$")

    def test_check_base_judges_what_a_change_did_to_the_proofs(self):
        self.edit("docs/pricing.md", "```pycon check", "```pycon compile")
        self.edit("docs/pricing.md", "<!-- fact: TAX_RATES -->", "<!-- fact: list(TAX_RATES) -->")
        self.edit(
            "docs/shipping.md",
            "These carriers deliver: <!-- fact @shipping: CARRIERS -->",
            "These carriers deliver:",
        )
        code, out, err = core("check", "--base", "HEAD", "--json", root=self.root)
        changes = {(c["type"], c["page"], c["severity"]) for c in json.loads(out)["changes"]}
        self.assertEqual(
            changes,
            {
                ("downgrade", "docs/pricing.md", "warning"),
                ("weakened", "docs/pricing.md", "warning"),
                ("orphaned-claim", "docs/shipping.md", "fail"),
            },
            err,
        )
        self.assertEqual(code, 1)

    def test_a_rename_is_not_a_weakening(self):
        for rel in ("src/shoplib/pricing.py", "src/shoplib/__init__.py"):
            self.edit(rel, "MAX_DISCOUNT", "DISCOUNT_CEILING")
        self.edit("docs/pricing.md", "<!-- fact: MAX_DISCOUNT -->", "<!-- fact: DISCOUNT_CEILING -->")
        code, out, err = core("check", "--base", "HEAD", "--json", root=self.root)
        self.assertEqual((code, json.loads(out)["changes"]), (0, []), err)


if __name__ == "__main__":
    unittest.main()
