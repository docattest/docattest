"""The environment attested code runs in: the import path, the surfaces, the snapshots and
manifests, no network unless a block asks for it."""

from __future__ import annotations

import builtins
import os
import sys
import threading
import types
from collections.abc import Sequence
from importlib.machinery import ModuleSpec
from typing import NoReturn

from .asserts import COMPARE, compare, compile_attested, run_code
from .channel import Sink
from .failures import failure_message
from .jobs import Settings, SetupModule, SurfaceToLoad

NETWORK_OFF = "network access is disabled for attestations; tag the block `check network` to allow it"
SETUP_MODULE = "docattest_setup"


class Environment:
    """What a job's child process is given: the import path, the surfaces, the snapshots and
    manifests."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        #: Errors raised in threads of attested code that nothing handled.
        self.strays: list[str] = []

    def enter(self, network: bool) -> None:
        settings = self.settings
        sys.path[:0] = [p for p in settings["path"] if p not in sys.path]
        if settings["aliases"]:
            sys.meta_path.insert(0, _AliasFinder(settings["aliases"]))
        sys.stdout = Sink()  # type: ignore[assignment]  # a file in all that print uses
        try:
            sys.stdin = open(os.devnull)
            os.dup2(sys.stdin.fileno(), 0)
        except OSError:
            pass
        setattr(builtins, COMPARE, compare)
        if not network:
            guard_network()
        threading.excepthook = self._stray

    def _stray(self, args: threading.ExceptHookArgs) -> None:
        if args.exc_type is not SystemExit and args.exc_value is not None:
            self.strays.append(failure_message(args.exc_value))

    def snapshot(self, name: str) -> object:
        import copy

        snapshots = self.settings["snapshots"]
        if name not in snapshots["data"]:
            where = snapshots["dir"] or "(no snapshots dir configured)"
            raise LookupError('snapshot "%s" not found in %s' % (name, where))
        return copy.deepcopy(snapshots["data"][name])

    def manifest(self, path: str) -> object:
        import copy

        # The core reads a manifest only when a fact writes its path out; one built at run
        # time was never read.
        manifests = self.settings["manifests"]
        if path not in manifests:
            raise LookupError(
                'manifest "%s" was not read: name it in the fact itself, as manifest("%s")' % (path, path)
            )
        return copy.deepcopy(manifests[path])

    def fresh_main(self, filename: str) -> dict[str, object]:
        main = types.ModuleType("__main__")
        main.__file__ = filename
        main.__builtins__ = builtins  # type: ignore[attr-defined]  # what a real __main__ has
        sys.modules["__main__"] = main
        return main.__dict__

    def load_setup(self, setup: SetupModule) -> types.ModuleType:
        """Run the setup module (its asserts rewritten like a block's) and return it."""
        existing = sys.modules.get(SETUP_MODULE)
        if existing is not None:
            return existing
        file = setup["file"]
        with open(file, encoding="utf-8") as handle:
            source = handle.read()
        directory = os.path.dirname(file)
        if directory not in sys.path:
            sys.path.insert(0, directory)
        module = types.ModuleType(SETUP_MODULE)
        module.__file__ = file
        module.snapshot = self.snapshot  # type: ignore[attr-defined]  # the setup module's own name
        code = compile_attested(source, file)
        sys.modules[SETUP_MODULE] = module
        try:
            run_code(code, module.__dict__)
        except BaseException:
            del sys.modules[SETUP_MODULE]
            raise
        return module


class _AliasFinder:
    """Imports of a surface configured as ``{"import": name, "path": file}``."""

    def __init__(self, aliases: dict[str, str]) -> None:
        # Imported here, not in find_spec: an import there would ask this finder again.
        import importlib.util

        self.aliases = aliases
        self.spec_from_file_location = importlib.util.spec_from_file_location

    def find_spec(
        self, fullname: str, path: Sequence[str] | None = None, target: types.ModuleType | None = None
    ) -> ModuleSpec | None:
        file = self.aliases.get(fullname)
        if file is None:
            return None
        package = os.path.basename(file) == "__init__.py"
        locations = [os.path.dirname(file)] if package else None
        return self.spec_from_file_location(fullname, file, submodule_search_locations=locations)


def guard_network() -> None:
    """Refuse internet sockets: connections, datagrams and name lookups. A socket pair is
    let through: where the platform has none of its own (Windows), Python makes one from a
    loopback connection, which reaches no other machine, and asyncio makes one for every
    event loop."""
    import socket

    def refuse(*_args: object, **_kwargs: object) -> NoReturn:
        raise OSError(NETWORK_OFF)

    inet = (socket.AF_INET, socket.AF_INET6)
    pairing = threading.local()

    def guarded(name: str) -> None:
        real = getattr(socket.socket, name)

        def method(self: socket.socket, *args: object, **kwargs: object) -> object:
            if self.family in inet and not getattr(pairing, "on", False):
                refuse()
            return real(self, *args, **kwargs)

        method.__name__ = name
        setattr(socket.socket, name, method)

    for name in ("connect", "connect_ex", "sendto"):
        guarded(name)
    real_pair = socket.socketpair

    def socketpair(*args: object, **kwargs: object) -> object:
        pairing.on = True
        try:
            return real_pair(*args, **kwargs)  # type: ignore[arg-type]
        finally:
            pairing.on = False

    socket.socketpair = socketpair
    socket.create_connection = refuse
    socket.getaddrinfo = refuse
    socket.gethostbyname = refuse


def surface_namespace(surface: SurfaceToLoad) -> dict[str, object]:
    """The names a fact may use: the surface's exports, and nothing else of the module."""
    import importlib

    module = importlib.import_module(surface["import_name"])
    names = surface["exports"]
    if names is None:
        names = getattr(module, "__all__", None) or [n for n in vars(module) if not n.startswith("_")]
    return {n: getattr(module, n) for n in names if hasattr(module, n)}
