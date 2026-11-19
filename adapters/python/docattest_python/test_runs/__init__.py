"""``runTests``: running the tests citations name with the project's own runner, pytest when
the project's interpreter has it and unittest otherwise, and reading how each one ended
from the runner's report.

``selection`` decides what runs and how (the command lines), ``pytest_report`` and
``unittest_report`` read what came back, and ``runner`` is the edge that starts the
processes; ``unittest_driver`` is the script unittest runs in, on the project's interpreter.
"""
