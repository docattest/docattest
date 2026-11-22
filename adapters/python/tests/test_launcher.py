"""The ``docattest`` command: where it finds the core, what it refuses, and what it hands
over. The npm launcher follows the same rules (``test/unit/launcher``)."""

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, PACKAGE_ROOT)

from docattest import launcher  # noqa: E402

PIN: launcher.Pin = {"version": "9.8.7", "protocol": 1}


def script(path: str, text: str) -> str:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write("#!/bin/sh\n" + text)
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
    return path


class Lookup(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.dir = os.path.realpath(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def find(self, env=None, bundled=None, ask=lambda core: None):
        bundled = bundled or os.path.join(self.dir, "none", "docattest")
        machine = launcher.Machine(
            env=env or {}, platform="linux", arch="x86_64", home=os.path.join(self.dir, "home")
        )
        return launcher.find_core(PIN, machine, bundled=bundled, ask=ask)

    def test_platforms_have_one_name_in_every_package(self):
        names = [
            launcher.platform_name(*pair)
            for pair in [("darwin", "arm64"), ("linux", "x86_64"), ("linux", "aarch64"), ("win32", "AMD64")]
        ]
        self.assertEqual(names, ["darwin-arm64", "linux-x64", "linux-arm64", "windows-x64"])

    def test_the_cache_is_the_users_cache_directory(self):
        cache = launcher.cache_directory
        self.assertEqual(cache({}, "darwin", "/Users/u"), "/Users/u/Library/Caches/docattest")
        self.assertEqual(cache({}, "linux", "/home/u"), "/home/u/.cache/docattest")
        self.assertEqual(cache({"XDG_CACHE_HOME": "/x"}, "linux", "/home/u"), "/x/docattest")
        self.assertEqual(
            cache({"XDG_CACHE_HOME": "relative"}, "linux", "/home/u"), "/home/u/.cache/docattest"
        )
        self.assertEqual(
            cache({"LOCALAPPDATA": "C:\\Users\\u\\AppData\\Local"}, "win32", "C:\\Users\\u"),
            "C:\\Users\\u\\AppData\\Local\\docattest",
        )
        self.assertEqual(cache({}, "win32", "C:\\Users\\u"), "C:\\Users\\u\\AppData\\Local\\docattest")
        self.assertEqual(
            cache({"DOCATTEST_CACHE": "/mine", "XDG_CACHE_HOME": "/x"}, "linux", "/home/u"), "/mine"
        )
        self.assertEqual(
            launcher.cached_core(PIN, {"DOCATTEST_CACHE": "/mine"}, "darwin", "arm64", "/h"),
            "/mine/core/9.8.7/darwin-arm64/docattest",
        )

    def test_the_core_is_the_one_given_then_the_bundled_one_then_the_cached_one(self):
        cache = os.path.join(self.dir, "cache")
        env = {"DOCATTEST_CACHE": cache}
        core, error = self.find(env)
        self.assertIsNone(core)
        self.assertEqual(
            error,
            f"docattest: no docattest core 9.8.7 for linux-x64. Looked in: DOCATTEST_CORE (not set); "
            f"this package ({self.dir}/none/docattest); the shared cache ({cache}/core/9.8.7/linux-x64/docattest)",
        )
        # Another version in the cache is not this package's core.
        script(os.path.join(cache, "core", "9.8.6", "linux-x64", "docattest"), "")
        self.assertIsNone(self.find(env)[0])
        cached = script(os.path.join(cache, "core", "9.8.7", "linux-x64", "docattest"), "")
        self.assertEqual(self.find(env), (cached, None))
        bundled = script(os.path.join(self.dir, "pkg", "bin", "docattest"), "")
        self.assertEqual(self.find(env, bundled=bundled), (bundled, None))
        # Neither of those is run to find it; a core someone points at is asked.
        asked = []
        given = dict(env, DOCATTEST_CORE="/given/core")

        def answer(core):
            asked.append(core)
            return '{"version":"1.0.0","protocol":1}'

        self.assertEqual(self.find(given, bundled=bundled, ask=answer), ("/given/core", None))
        self.assertEqual(asked, ["/given/core"])

    def test_a_core_that_speaks_another_protocol_is_refused(self):
        env = {"DOCATTEST_CORE": "/given/core"}
        core, error = self.find(env, ask=lambda core: '{"version":"2.0.0","protocol":2}')
        self.assertIsNone(core)
        self.assertEqual(
            error,
            "docattest: DOCATTEST_CORE (/given/core) is core 2.0.0, which speaks adapter protocol 2; "
            "this package speaks protocol 1 (it was built for core 9.8.7)",
        )
        for answer in (None, "", "docattest 1.0", '{"version":"1"}', "[1]"):
            core, error = self.find(env, ask=lambda core, answer=answer: answer)
            self.assertEqual(
                (core, error),
                (
                    None,
                    "docattest: DOCATTEST_CORE (/given/core) is not a docattest core: it does not answer --version --json",
                ),
            )

    def test_with_no_core_it_says_why_and_how_to_get_one(self):
        def advice(name):
            return launcher.remedy("9.8.7", name, ["darwin-arm64", "linux-x64"])

        core, error = launcher.find_core(
            PIN,
            launcher.Machine(
                env={"DOCATTEST_CACHE": self.dir}, platform="linux", arch="x86_64", home=self.dir
            ),
            bundled="",
            advice=advice,
        )
        self.assertIsNone(core)
        assert error is not None
        first, second = error.split("\n")
        self.assertTrue(
            first.startswith("docattest: no docattest core 9.8.7 for linux-x64. Looked in:"), first
        )
        self.assertEqual(
            second,
            "A docattest wheel for linux-x64 carries the core as docattest/bin/docattest, and this installation has none: "
            "it was installed from source (a checkout, or a wheel built without the core). To fix it, install the wheel "
            "(pip install --force-reinstall --only-binary :all: docattest==9.8.7), or set DOCATTEST_CORE to a docattest core 9.8.7.",
        )
        _, error = launcher.find_core(
            PIN,
            launcher.Machine(
                env={"DOCATTEST_CACHE": self.dir}, platform="linux", arch="ppc64le", home=self.dir
            ),
            bundled="",
            advice=advice,
        )
        assert error is not None
        self.assertEqual(
            error.split("\n")[1],
            "docattest brings a core for darwin-arm64, linux-x64; there is none for linux-ppc64le. Build one from docattest's "
            "source (cargo build --release in its core/ directory) and set DOCATTEST_CORE to it.",
        )
        # A core someone points at, and refused, is not a missing core: no advice.
        _, error = launcher.find_core(
            PIN, launcher.Machine(env={"DOCATTEST_CORE": "/given/core"}), ask=lambda core: None, advice=advice
        )
        assert error is not None
        self.assertNotIn("\n", error)

    def test_the_package_pins_the_core_it_was_built_for(self):
        with open(
            os.path.join(PACKAGE_ROOT, "..", "..", "protocol", "core.json"), encoding="utf-8"
        ) as handle:
            self.assertEqual(launcher.pinned(), json.load(handle))


class Launch(unittest.TestCase):
    """The launcher run as a program, with a stand-in core that reports what it was given."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.dir = os.path.realpath(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        self.core = script(
            os.path.join(self.dir, "my tools", "core"),
            'case "$*" in "--version --json") echo \'{"version":"0.0.0","protocol":1}\'; exit 0;; esac\n'
            'printf "%s\\n" "$DOCATTEST_ADAPTER_PYTHON" "${DOCATTEST_COMMAND-unset}" "$#" "$@"\ncat\nexit 7\n',
        )

    def launch(self, *args, env=None, cwd=None, stdin=""):
        environment = {k: v for k, v in os.environ.items() if not k.startswith("DOCATTEST_")}
        environment.update(PYTHONPATH=PACKAGE_ROOT, DOCATTEST_CORE=self.core)
        environment.update(env or {})
        done = subprocess.run(
            [sys.executable, "-m", "docattest", *args],
            input=stdin,
            capture_output=True,
            text=True,
            env=environment,
            cwd=cwd or self.dir,
            timeout=60,
        )
        return done.returncode, done.stdout.split("\n"), done.stderr

    def test_arguments_streams_and_the_exit_code_are_the_cores(self):
        code, out, err = self.launch("check", "a b", "--json", stdin="from stdin")
        self.assertEqual((code, err), (7, ""))
        adapter = json.loads(out[0])
        self.assertEqual(
            adapter, [sys.executable, os.path.join(PACKAGE_ROOT, "docattest_python", "__main__.py")]
        )
        self.assertEqual(out[1:], ["unset", "3", "check", "a b", "--json", "from stdin"])

    def test_a_trace_names_the_core_it_runs(self):
        code, _, err = self.launch("check", env={"DOCATTEST_TRACE": "1"})
        self.assertEqual((code, err), (7, f"[trace launcher] core {self.core}\n"))

    def test_a_trace_variable_set_to_nothing_asks_for_no_trace(self):
        code, _, err = self.launch("check", env={"DOCATTEST_TRACE": ""})
        self.assertEqual((code, err), (7, ""))

    def test_with_no_core_the_platforms_are_read_from_the_package(self):
        code, _, err = self.launch(
            "check", env={"DOCATTEST_CORE": "", "DOCATTEST_CACHE": os.path.join(self.dir, "cache")}
        )
        self.assertEqual(code, 1)
        self.assertEqual(len(err.rstrip("\n").split("\n")), 2, err)
        self.assertIn("set DOCATTEST_CORE to", err.split("\n")[1])

    def test_the_adapter_it_names_starts_and_greets(self):
        _, out, _ = self.launch("check")
        hello = subprocess.run(
            json.loads(out[0]),
            input='{"id":1,"method":"hello","params":{"protocol":1}}\n',
            capture_output=True,
            text=True,
            cwd=self.dir,
            timeout=60,
        )
        self.assertEqual(json.loads(hello.stdout)["result"]["adapter"]["name"], "python")

    def test_what_is_already_set_stands(self):
        _, out, _ = self.launch(
            "init",
            "--hooks",
            env={"DOCATTEST_ADAPTER_PYTHON": "mine", "DOCATTEST_COMMAND": "uv run docattest"},
        )
        self.assertEqual(out[:2], ["mine", "uv run docattest"])

    def test_hooks_are_told_a_command_that_runs_docattest_without_node(self):
        _, out, _ = self.launch("init", "--hooks")
        command = out[1]
        self.assertNotIn("npx", command)
        self.assertRegex(command, r"\bdocattest\b")
        # In a virtualenv inside the project it is the environment's own script, by a path
        # that a settings file can share between machines.
        project = os.path.join(self.dir, "my project")
        script(os.path.join(project, ".venv", "bin", "docattest"), "")
        import shlex

        probe = (
            "import sys; sys.prefix, sys.base_prefix = sys.argv[1], '/usr'; "
            "from docattest import launcher; print(launcher.docattest_command(sys.argv[2]))"
        )
        done = subprocess.run(
            [sys.executable, "-c", probe, os.path.join(project, ".venv"), project],
            capture_output=True,
            text=True,
            env=dict(os.environ, PYTHONPATH=PACKAGE_ROOT),
            timeout=60,
        )
        # The launcher knows no agent: the root is a placeholder the core fills in.
        self.assertEqual(done.stdout.strip(), "{root}/.venv/bin/docattest", done.stderr)
        self.assertEqual(
            shlex.split(done.stdout.replace("{root}", shlex.quote(project))),
            [os.path.join(project, ".venv", "bin", "docattest")],
        )

    def test_every_command_that_writes_the_command_down_is_told_it(self):
        for args in (["init", "--hooks"], ["init", "--git-hook"], ["init", "--agents"]):
            _, out, _ = self.launch(*args)
            self.assertRegex(out[1], r"\bdocattest\b", args)
        for args in (["check"], ["impact", "--file", "a.py"], ["audit"]):
            _, out, _ = self.launch(*args)
            self.assertEqual(out[1], "unset", args)

    def test_the_root_is_found_as_the_core_finds_it(self):
        from docattest import launcher

        project = os.path.join(self.dir, "found")
        os.makedirs(os.path.join(project, "src", "deep"))
        with open(os.path.join(project, "docattest.config.json"), "w") as f:
            f.write("{}")
        before = os.getcwd()
        self.addCleanup(os.chdir, before)
        os.chdir(os.path.join(project, "src", "deep"))
        self.assertEqual(launcher._root(["init", "--hooks"]), project)
        self.assertEqual(
            launcher._root(["init", "--hooks", "--root", "."]), os.path.join(project, "src", "deep")
        )
        self.assertEqual(launcher._root(["init", "--root=..", "--hooks"]), os.path.join(project, "src"))
        os.chdir(self.dir)
        self.assertEqual(launcher._root(["init", "--hooks"]), self.dir)

    def test_a_missing_core_names_the_three_places(self):
        code, _, err = self.launch(
            "check", env={"DOCATTEST_CORE": "", "DOCATTEST_CACHE": os.path.join(self.dir, "cache")}
        )
        self.assertEqual(code, 1)
        for place in (
            "DOCATTEST_CORE (not set)",
            "this package (",
            f"the shared cache ({self.dir}/cache/core/",
        ):
            self.assertIn(place, err)


if __name__ == "__main__":
    unittest.main()
