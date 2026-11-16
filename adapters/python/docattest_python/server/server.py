"""The protocol server: one JSON request per line on stdin, one JSON response per line on
stdout (PROTOCOL.md).

``hello`` is answered by the thread that reads stdin, before anything heavier than ``json``
is imported. Every other request is handled, in arrival order, by one worker thread, so
``open`` has taken effect before the request after it and the session's caches need no
locks; a request whose work runs in the process pool is answered when that work is done,
which is how answers overtake each other.
"""

from __future__ import annotations

import json
import os
import queue
import sys
import threading
from collections.abc import Callable
from types import ModuleType
from typing import TYPE_CHECKING, cast

from ..errors import ProtocolError, UserError
from ..workers.docattest_worker.fields import Fields, MalformedMessage

if TYPE_CHECKING:
    # Imported by the handler thread: ``hello`` is answered before any of them is.
    from concurrent.futures import Future

    from ..session.session import Session
    from .params import Params

PROTOCOL_VERSION = 1
#: How much of a request line a message quotes.
PREVIEW_LENGTH = 80

#: A request's id: a number or a string.
RequestId = int | float | str | None


class Writer:
    """The only thing that writes to the real stdout."""

    def __init__(self, fd: int) -> None:
        self._out = os.fdopen(fd, "wb", buffering=0)
        self._lock = threading.Lock()

    def send(self, message: dict[str, object]) -> None:
        data = (json.dumps(message) + "\n").encode("utf-8")
        with self._lock:
            try:
                self._out.write(data)
            except (OSError, ValueError):
                pass  # the core went away; there is nobody to tell


def _preview(line: str) -> str:
    return line[:PREVIEW_LENGTH] + "…" if len(line) > PREVIEW_LENGTH else line


class Server:
    def __init__(self, writer: Writer) -> None:
        self.writer = writer
        self.greeted = False
        self.session: Session | None = None
        self.closing = False
        self.tasks: queue.Queue[Callable[[], None] | None] = queue.Queue()
        self.thread: threading.Thread | None = None
        self.pending = 0
        self.settled = threading.Condition()

    # --- reading --------------------------------------------------------------------------

    def line(self, raw: bytes) -> None:
        text = raw.decode("utf-8", errors="replace").strip("\r\n")
        if not text.strip():
            return
        try:
            request: object = json.loads(text)
        except ValueError:
            return self._error(None, ProtocolError(f"a request line is not JSON: {_preview(text)}"))
        if not isinstance(request, dict):
            return self._error(None, ProtocolError(f"a request is not an object: {_preview(text)}"))
        fields = cast("dict[str, object]", request)
        rid = fields.get("id")
        if isinstance(rid, bool) or not isinstance(rid, (int, float, str)):
            return self._error(
                None, ProtocolError(f'a request has no "id" (a number or a string): {_preview(text)}')
            )
        method = fields.get("method")
        params = fields.get("params", {})
        if not isinstance(method, str):
            return self._error(rid, ProtocolError(f'request {rid} has no "method"'))
        if not isinstance(params, dict):
            return self._error(rid, ProtocolError(f'request {rid}: "params" is not an object'))
        params = cast("dict[str, object]", params)
        if method == "hello":
            return self._hello(rid, Fields(params, method))
        # Whether hello came first is a matter of arrival order, not of when this runs.
        greeted = self.greeted
        self._enqueue(lambda: self._handle(rid, method, params, greeted))

    def _hello(self, rid: RequestId, params: Fields) -> None:
        try:
            protocol = params.number("protocol")
            warm = params.has("warm") and params.boolean("warm")
        except MalformedMessage as err:
            return self._error(rid, err)
        if protocol != PROTOCOL_VERSION:
            shown = int(protocol) if float(protocol).is_integer() else protocol
            return self._error(
                rid, UserError(f"the python adapter speaks protocol {PROTOCOL_VERSION}, not {shown}")
            )
        from .. import language

        self.greeted = True
        self.writer.send({"id": rid, "result": language.hello()})
        if warm:
            # More requests follow: load what they need while the core reads the project.
            self._enqueue(_warm)

    # --- the handler thread ---------------------------------------------------------------

    def _enqueue(self, task: Callable[[], None]) -> None:
        if self.thread is None:
            self.thread = threading.Thread(target=self._serve, daemon=True)
            self.thread.start()
        self.tasks.put(task)

    def _serve(self) -> None:
        while True:
            task = self.tasks.get()
            if task is None:
                return
            task()

    def _handle(self, rid: RequestId, method: str, params: dict[str, object], greeted: bool) -> None:
        from concurrent.futures import Future

        from .params import Params

        with self.settled:
            self.pending += 1
        try:
            answer = self._call(method, Params(method, params), greeted)
        except BaseException as err:  # noqa: BLE001
            self._error(rid, err)
            self._settle()
            return
        if isinstance(answer, Future):
            future = cast("Future[object]", answer)
            future.add_done_callback(lambda done: self._answer(rid, done))
        else:
            self.writer.send({"id": rid, "result": answer})
            self._settle()

    def _answer(self, rid: RequestId, future: Future[object]) -> None:
        try:
            result = future.result()
        except BaseException as err:  # noqa: BLE001
            self._error(rid, err)
        else:
            self.writer.send({"id": rid, "result": result})
        self._settle()

    def _settle(self) -> None:
        with self.settled:
            self.pending -= 1
            self.settled.notify_all()

    def _wait_for_others(self, own: int) -> None:
        """Wait until every request but ``own`` of them has its answer."""
        with self.settled:
            while self.pending > own:
                self.settled.wait()

    def _error(self, rid: RequestId, err: BaseException) -> None:
        if isinstance(err, UserError):
            error = {"kind": "user", "message": str(err)}
        elif isinstance(err, (ProtocolError, MalformedMessage)):
            error = {"kind": "internal", "message": str(err)}
        else:
            import traceback

            detail = "".join(traceback.format_exception(type(err), err, err.__traceback__)).strip()
            error = {"kind": "internal", "message": detail}
        self.writer.send({"id": rid, "error": error})

    # --- methods --------------------------------------------------------------------------

    def _call(self, method: str, p: Params, greeted: bool) -> object:
        text = TEXT_METHODS.get(method)
        handler = SESSION_METHODS.get(method)
        if text is None and handler is None and method not in ("open", "close"):
            raise ProtocolError(f'unknown method "{method}"')
        if not greeted:
            raise ProtocolError(f'"{method}" before "hello": hello is always first')
        if text is not None:
            from .. import language

            return text(language, p)
        if method == "open":
            return self._open(p)
        if self.session is None:
            raise ProtocolError(f'"{method}" before "open"')
        if self.closing:
            raise ProtocolError(f'"{method}" after "close"')
        if handler is None:
            return self._close(self.session)
        return handler(self.session, p)

    def _open(self, p: Params) -> dict[str, object]:
        if self.session is not None:
            raise ProtocolError('"open" was already called: one session per process')
        from ..session.session import Session

        self.session = Session(p.session_settings())
        return {}

    def _close(self, session: Session) -> dict[str, object]:
        # Every request already sent has its answer before the workers are released.
        self.closing = True
        self._wait_for_others(1)
        session.close()
        return {}

    def finish(self) -> None:
        """End of input: answer what is in flight, close the session, stop."""
        done = threading.Event()
        if self.thread is not None:
            self.tasks.put(done.set)
            done.wait()
        self._wait_for_others(0)
        self.shutdown()

    def shutdown(self) -> None:
        if self.session is not None and not self.closing:
            self.closing = True
            self.session.close()


def _warm() -> None:
    import importlib

    importlib.import_module("..session.session", __package__)  # ast, subprocess and the rest


#: The questions about text, answered without a session by ``language``.
TEXT_METHODS: dict[str, Callable[[ModuleType, Params], object]] = {
    "docComments": lambda language, p: language.doc_comments(p.sources("files")),
    "assertions": lambda language, p: language.assertions(p.snippets("examples")),
    "docProse": lambda language, p: language.doc_prose(p.sources("files")),
    "mentions": lambda language, p: language.mentions(p.strings("spans"), p.strings("symbols")),
    "expressions": lambda language, p: language.expressions(p.strings("expressions")),
}

#: The questions about the project, answered by the session ``open`` created.
SESSION_METHODS: dict[str, Callable[[Session, Params], object]] = {
    "compile": lambda s, p: s.problems.compile(p.examples("examples")),
    "evaluate": lambda s, p: s.running.evaluate(p.facts("facts")),
    "run": lambda s, p: s.running.run(p.examples("examples"), p.cases("cases")),
    "symbols": lambda s, p: s.symbols.symbols(p.attestations("attestations")),
    "exports": lambda s, p: s.workspace.exports(p.string("surface")),
    "describeSurfaces": lambda s, p: s.describe_surfaces(),
    "tests": lambda s, p: s.symbols.tests(p.string("file")),
    "observables": lambda s, p: s.running.observables(p.string("surface")),
    "affected": lambda s, p: s.affected(p.base("base"), p.strings("changed")),
    "inCode": lambda s, p: s.workspace.in_code(p.strings("names"), p.strings("files")),
}


def main() -> None:
    # Descriptor 1 becomes stderr for this process and every process it starts, so nothing
    # but responses can reach the core: they are written to a private copy of the pipe.
    sys.stdout.flush()
    private = os.dup(1)
    os.dup2(2, 1)
    os.set_inheritable(private, False)
    sys.stdout = sys.stderr
    server = Server(Writer(private))

    def stop(signum: int, _frame: object) -> None:
        server.shutdown()
        os._exit(128 + signum)

    try:
        import signal

        for name in ("SIGTERM", "SIGINT", "SIGHUP"):
            if hasattr(signal, name):
                signal.signal(getattr(signal, name), stop)
    except (ValueError, OSError):
        pass
    for raw in sys.stdin.buffer:
        server.line(raw)
    server.finish()
    sys.stderr.flush()
    os._exit(0)
