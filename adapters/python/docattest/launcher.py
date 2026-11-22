"""The ``docattest`` command of the Python package: a launcher for the docattest core.

The core is one binary for every language and knows nothing of how it was installed. This
launcher belongs to the language package. It hands the core two things in the environment
(PROTOCOL.md, section 1, "Launchers") and then becomes it (``exec``; on Windows, which has
no exec, it runs the core and exits with its code), so arguments, standard streams, signals
and the exit code are the core's own:

``DOCATTEST_ADAPTER_PYTHON``
    How to start this package's adapter: a JSON array, the program and its arguments.
``DOCATTEST_COMMAND``
    A shell command that runs docattest in this project; ``hooks install`` and ``agents
    install`` write it down. ``{root}`` in it stands for the project root, which the core
    spells as whoever will run the command spells it.

A variable that is already set is left alone. Only the standard library is imported, and
as little of it as will do: this runs before every docattest command.
"""

from __future__ import annotations

import os
import sys
from collections.abc import Callable, Mapping
from typing import TypedDict, cast

CORE_VARIABLE = "DOCATTEST_CORE"
ADAPTER_VARIABLE = "DOCATTEST_ADAPTER_PYTHON"
COMMAND_VARIABLE = "DOCATTEST_COMMAND"
# The core's own trace, which this launcher answers too: set to anything but nothing.
TRACE_VARIABLE = "DOCATTEST_TRACE"
# In the command, the project root: the core says how it is spelt where the command is written.
ROOT = "{root}"
# The commands that write the command down; only for them is it worked out.
WRITERS = ("init",)

_HERE = os.path.dirname(os.path.abspath(__file__))


class Pin(TypedDict):
    """``protocol/core.json``: the core this package was built for."""

    version: str
    protocol: int


class Platform(TypedDict):
    """One platform a core is built for, in ``protocol/platforms.json``."""

    os: str
    cpu: str
    rust: str
    wheel: list[str]
    binary: str


class Platforms(TypedDict):
    """``protocol/platforms.json``: the platforms a core is built for, by ``<os>-<cpu>``."""

    npmPackage: str
    platforms: dict[str, Platform]


#: The environment, as finding the core reads it.
Environment = Mapping[str, str]
#: Asks a program for ``--version --json``; None when it does not answer.
AskCore = Callable[[str], str | None]
#: What a user with no core is told to do, given the platform's name.
Advice = Callable[[str], str]


def _protocol_file(name: str) -> object:
    """A file of the package's copy of ``protocol/`` (written by ``sync_protocol.py``)."""
    import json

    with open(
        os.path.join(os.path.dirname(_HERE), "docattest_python", "protocol", name), encoding="utf-8"
    ) as f:
        return json.load(f)


def pinned() -> Pin:
    """The core this package was built for: its exact version and the protocol it speaks.
    Copied from ``protocol/core.json`` when the package was built."""
    return cast(Pin, _protocol_file("core.json"))


def remedy(version: str, platform: str, listed: list[str]) -> str:
    """What a user with no core is told: why this installation has none, and the ways to get
    one. ``listed`` are the platforms a wheel with a core exists for (``protocol/platforms.json``)."""
    if platform not in listed:
        return (
            f"docattest brings a core for {', '.join(listed)}; there is none for {platform}. "
            f"Build one from docattest's source (cargo build --release in its core/ directory) and "
            f"set {CORE_VARIABLE} to it."
        )
    return (
        f"A docattest wheel for {platform} carries the core as docattest/bin/docattest, and this "
        f"installation has none: it was installed from source (a checkout, or a wheel built "
        f"without the core). To fix it, install the wheel "
        f"(pip install --force-reinstall --only-binary :all: docattest=={version}), "
        f"or set {CORE_VARIABLE} to a docattest core {version}."
    )


def platform_name(platform: str, arch: str) -> str:
    """``<os>-<arch>`` in the names every package uses: ``darwin-arm64``, ``linux-x64``."""
    system = "windows" if platform == "win32" else platform
    cpu = arch.lower()
    return f"{system}-{ {'x86_64': 'x64', 'amd64': 'x64', 'aarch64': 'arm64'}.get(cpu, cpu) }"


def _arch() -> str:
    if hasattr(os, "uname"):
        return os.uname().machine
    return os.environ.get("PROCESSOR_ARCHITECTURE", "unknown")


def cache_directory(env: Environment, platform: str, home: str) -> str:
    """docattest's own cache directory: ``DOCATTEST_CACHE``, else ``docattest`` in the
    user's cache directory (``~/Library/Caches`` on macOS, ``%LOCALAPPDATA%`` on Windows,
    ``$XDG_CACHE_HOME`` or ``~/.cache`` elsewhere)."""
    if env.get("DOCATTEST_CACHE"):
        return env["DOCATTEST_CACHE"]
    if platform == "win32":
        import ntpath

        return ntpath.join(env.get("LOCALAPPDATA") or ntpath.join(home, "AppData", "Local"), "docattest")
    import posixpath

    if platform == "darwin":
        return posixpath.join(home, "Library", "Caches", "docattest")
    xdg = env.get("XDG_CACHE_HOME")
    return posixpath.join(
        xdg if xdg and posixpath.isabs(xdg) else posixpath.join(home, ".cache"), "docattest"
    )


def cached_core(pin: Pin, env: Environment, platform: str, arch: str, home: str) -> str:
    """Where a core of the pinned version lies in the shared cache."""
    sep = "\\" if platform == "win32" else "/"
    name = "docattest.exe" if platform == "win32" else "docattest"
    parts = [
        cache_directory(env, platform, home),
        "core",
        pin["version"],
        platform_name(platform, arch),
        name,
    ]
    return sep.join(parts)


def ask_core(core: str) -> str | None:
    """``core --version --json``, or None when the program does not answer it."""
    import subprocess

    try:
        done = subprocess.run([core, "--version", "--json"], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    return done.stdout if done.returncode == 0 else None


def _checked(core: str, pin: Pin, ask: AskCore) -> tuple[str | None, str | None]:
    import json

    not_a_core = (
        None,
        f"docattest: {CORE_VARIABLE} ({core}) is not a docattest core: it does not answer --version --json",
    )
    try:
        told: object = json.loads(ask(core) or "")
    except ValueError:
        return not_a_core
    if not isinstance(told, dict):
        return not_a_core
    answer = cast("dict[str, object]", told)
    version, protocol = answer.get("version"), answer.get("protocol")
    if not isinstance(version, str) or isinstance(protocol, bool) or not isinstance(protocol, (int, float)):
        return not_a_core
    if protocol != pin["protocol"]:
        return None, (
            f"docattest: {CORE_VARIABLE} ({core}) is core {version}, which speaks adapter protocol "
            f"{protocol}; this package speaks protocol {pin['protocol']} (it was built for core {pin['version']})"
        )
    return core, None


class Machine:
    """What finding the core reads of the machine it runs on: its environment, its operating
    system as ``sys.platform`` names it, its CPU and its home directory. Each is this
    machine's own unless given; the tests describe other machines."""

    def __init__(
        self,
        env: Environment | None = None,
        platform: str | None = None,
        arch: str | None = None,
        home: str | None = None,
    ) -> None:
        self.env = os.environ if env is None else env
        self.platform = sys.platform if platform is None else platform
        self.arch = _arch() if arch is None else arch
        self.home = os.path.expanduser("~") if home is None else home


def find_core(
    pin: Pin,
    machine: Machine | None = None,
    bundled: str | None = None,
    ask: AskCore = ask_core,
    advice: Advice | None = None,
) -> tuple[str | None, str | None]:
    """The core, in order of preference: ``DOCATTEST_CORE``; the binary the package brought
    (a platform wheel ships it as ``docattest/bin/docattest``); the shared cache. The last
    two are trusted without being run: the package brought the first itself, and the cache
    path names the exact version. A core someone else points at is asked what protocol it
    speaks, and refused when it is not this package's. Returns ``(core, error)``; when there
    is no core at all, ``advice(platform name)`` is added to the error on a line of its own.

    The npm launcher (``adapters/typescript/src/core-binary/locate.ts``) follows the same rules, so
    that an npm install and a pip install on one machine can share one binary."""
    machine = Machine() if machine is None else machine
    given = machine.env.get(CORE_VARIABLE)
    if given:
        return _checked(given, pin, ask)
    if bundled is None:
        bundled = os.path.join(_HERE, "bin", "docattest.exe" if machine.platform == "win32" else "docattest")
    if bundled and os.path.isfile(bundled):
        return bundled, None
    cached = cached_core(pin, machine.env, machine.platform, machine.arch, machine.home)
    if os.path.isfile(cached):
        return cached, None
    return None, _no_core(pin, platform_name(machine.platform, machine.arch), bundled, cached, advice)


def _no_core(pin: Pin, name: str, bundled: str, cached: str, advice: Advice | None) -> str:
    """The error when there is no core anywhere: every place it was looked for."""
    brought = bundled or "it brought no binary for this platform"
    told = "" if advice is None else "\n" + advice(name)
    return (
        f"docattest: no docattest core {pin['version']} for {name}. Looked in: "
        f"{CORE_VARIABLE} (not set); this package ({brought}); the shared cache ({cached}){told}"
    )


def _platform_names() -> list[str]:
    """The platforms a wheel with a core exists for."""
    return list(cast(Platforms, _protocol_file("platforms.json"))["platforms"])


def adapter_command() -> list[str]:
    """This interpreter, running the adapter that was installed next to this launcher. The
    adapter is named by its file, so nothing in the working directory can stand in for it."""
    adapter = os.path.join(os.path.dirname(_HERE), "docattest_python", "__main__.py")
    return [sys.executable, adapter]


CONFIG_FILE = "docattest.config.json"


def _root(argv: list[str]) -> str:
    """The project root the core will use: ``--root``, else the nearest directory at or
    above the working directory that holds a ``docattest.config.json``, else the working
    directory."""
    root = None
    for k, arg in enumerate(argv):
        if arg == "--root" and k + 1 < len(argv):
            root = argv[k + 1]
        elif arg.startswith("--root="):
            root = arg[len("--root=") :]
    if root is not None:
        return os.path.realpath(root)
    here = directory = os.path.realpath(".")
    while not os.path.isfile(os.path.join(directory, CONFIG_FILE)):
        above = os.path.dirname(directory)
        if above == directory:
            return here
        directory = above
    return directory


def docattest_command(root: str) -> str:
    """A shell command that runs docattest in the project at ``root``, for a hook that is
    started with no virtualenv active.

    In a virtualenv inside the project (the usual ``.venv``) it is the environment's own
    script, relative to the project: ``{root}/.venv/bin/docattest``, which a file can share
    between machines. What ``{root}`` becomes is not this launcher's to know: the core
    writes there how the reader of the command names the project root (a coding agent's own
    variable in that agent's hook, ``.`` in ``AGENTS.md`` and in a git hook). Otherwise the
    command is ``docattest`` when that is what the shell finds on the ``PATH``, and else
    this interpreter by its full path.
    """
    import shlex

    prefix = os.path.realpath(sys.prefix)
    script = os.path.join(prefix, "Scripts" if os.name == "nt" else "bin", "docattest")
    in_virtualenv = sys.prefix != getattr(sys, "base_prefix", sys.prefix)
    inside = os.path.commonpath([prefix, root]) == root and prefix != root
    if in_virtualenv and inside and os.path.isfile(script):
        relative = os.path.relpath(script, root).replace(os.sep, "/")
        return ROOT + "/" + shlex.quote(relative)
    import shutil

    found = shutil.which("docattest")
    if found and os.path.isfile(script) and os.path.samefile(found, script):
        return "docattest"
    if os.path.isfile(script):
        return shlex.quote(script)
    return f"{shlex.quote(sys.executable)} -m docattest"


def environment(argv: list[str]) -> dict[str, str]:
    """The core's environment: this process's, with the hand-off added where unset."""
    import json

    env = dict(os.environ)
    if not env.get(ADAPTER_VARIABLE):
        env[ADAPTER_VARIABLE] = json.dumps(adapter_command())
    # Only these commands read it, and working it out looks at the file system.
    if not env.get(COMMAND_VARIABLE) and any(writer in argv for writer in WRITERS):
        env[COMMAND_VARIABLE] = docattest_command(_root(argv))
    return env


def main() -> None:
    argv = sys.argv[1:]
    pin = pinned()
    core, error = find_core(pin, advice=lambda name: remedy(pin["version"], name, _platform_names()))
    if core is None:
        sys.stderr.write(f"{error}\n")
        sys.exit(1)
    # With a trace asked for, say which core runs.
    if os.environ.get(TRACE_VARIABLE):
        sys.stderr.write(f"[trace launcher] core {core}\n")
    try:
        if sys.platform == "win32":
            # Windows has no exec: os.execve starts the core and ends this process at once,
            # so whoever ran docattest would see neither the core's exit code nor wait for it.
            import subprocess

            sys.exit(subprocess.call([core, *argv], env=environment(argv)))
        os.execve(core, [core, *argv], environment(argv))
    except OSError as err:
        sys.stderr.write(f"docattest: cannot run the core ({core}): {err}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
