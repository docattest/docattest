"""``python -m docattest_python``, the ``docattest-adapter-python`` command, and this file
run by its path (which is how the ``docattest`` launcher starts the adapter)."""

if __package__:
    from .server.server import main
else:
    # Run as a file: the first entry of sys.path is this directory. Make it the directory
    # that holds the package instead, so that the working directory is never searched.
    import os
    import sys

    sys.path[0] = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    from docattest_python.server.server import main

if __name__ == "__main__":
    main()
