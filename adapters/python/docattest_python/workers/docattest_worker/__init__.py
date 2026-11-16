"""The worker's parts; ``worker.py`` beside this package is its entry point.

The worker imports this package by its own name, from the directory it was started in,
before it takes that directory off the import path. Like the worker, it uses only the
standard library, so it also runs on the project's own interpreter."""
