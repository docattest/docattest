"""docattest for Python projects: the ``docattest`` command.

This package is the launcher. It finds the docattest core it was installed with, tells it
how to start the Python adapter (``docattest_python``) and what command runs docattest in
this project, and replaces itself with the core (PROTOCOL.md, section 1, "Launchers").
"""

from docattest_python import __version__

__all__ = ["__version__"]
