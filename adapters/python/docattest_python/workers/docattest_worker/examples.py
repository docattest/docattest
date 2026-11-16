"""The example job: a block run as a script, after the blocks it continues."""

from __future__ import annotations

from .asserts import compile_attested, run_code
from .channel import Channel
from .environment import Environment
from .failures import describe_error, failure_message
from .jobs import BlockJob, ContinuedBlock
from .messages import Verdict


def run_example(job: BlockJob, env: Environment, channel: Channel) -> None:
    filename, source = job["file"], job["code"]
    env.enter(job["network"])
    namespace = env.fresh_main(filename)
    broken = run_before(job["before"], namespace)
    if broken:
        channel.post({"t": "verdict", "verdict": {"status": "fail", "message": broken}})
        return
    try:
        run_code(compile_attested(source, filename), namespace)
    except BaseException as err:  # noqa: BLE001 - whatever the block raised is its verdict
        verdict: Verdict = {**describe_error(err, filename, source), "status": "fail"}
    else:
        verdict = completed(env, job["weak"], "all assertions held")
    channel.post({"t": "verdict", "verdict": verdict})


def completed(env: Environment, weak: bool, held: str) -> Verdict:
    """The verdict on a block that ran to its end: ``held`` says what it proved."""
    if env.strays:
        return {
            "status": "fail",
            "message": "unhandled error from attested code: " + "; ".join(env.strays),
        }
    if weak:
        return {"status": "pass", "message": "ran without raising, but asserts nothing (weak)"}
    return {"status": "pass", "message": held}


def run_before(before: list[ContinuedBlock], namespace: dict[str, object]) -> str | None:
    """Run the blocks this one continues (``continue`` in its info string), in page order,
    in the namespace it will run in: one session, as a reader of the page takes it. Each
    block still runs in a process of its own, so what came before is run again for it.

    What an earlier block proves is its own result; here only what it leaves behind
    matters. A session's outputs are therefore not compared, and a statement it expects to
    raise may raise. Returns a message when an earlier block cannot be run."""
    for block in before:
        filename, source = block["file"], block["code"]
        try:
            if block["session"]:
                # Imported only when a session is run: doctest is slow to import.
                from .sessions import run_quietly

                run_quietly(source, namespace, filename)
            else:
                run_code(compile_attested(source, filename), namespace)
        except BaseException as err:  # noqa: BLE001
            return "the block it continues (L%d) did not run: %s" % (block["line"], failure_message(err))
    return None
