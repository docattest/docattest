"""Copy the protocol's neutral files into the package: ``docattest_python/protocol/``.

    python3 adapters/python/sync_protocol.py

The schemas, the agent descriptors, ``core.json`` (the core version and protocol this
package is built for) and ``platforms.json`` (the platforms a core is built for) have one
source, ``protocol/`` at the root of the repository. The package ships a copy, made here
(and by ``setup.py`` whenever the package is built from a checkout), never by hand;
``tests/test_schema.py`` fails when the copy differs.

``protocol/sync.ts`` makes the same copy for the npm package. This is a second one, in
Python, because the wheel is built with nothing but Python and pip: a build of it must not
need Node. Either way the copy must equal ``protocol/`` byte for byte, which is what
``tests/test_schema.py`` checks.
"""

import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.normpath(os.path.join(HERE, "..", "..", "protocol"))
TARGET = os.path.join(HERE, "docattest_python", "protocol")


def sync() -> bool:
    """Copy when the source is there (a checkout); an sdist already holds the copy."""
    if not os.path.isfile(os.path.join(SOURCE, "core.json")):
        return False
    for directory in ("schema", "agents"):
        shutil.rmtree(os.path.join(TARGET, directory), ignore_errors=True)
        os.makedirs(os.path.join(TARGET, directory))
        for name in sorted(os.listdir(os.path.join(SOURCE, directory))):
            if name.endswith(".json"):
                shutil.copyfile(os.path.join(SOURCE, directory, name), os.path.join(TARGET, directory, name))
    for name in ("core.json", "platforms.json"):
        shutil.copyfile(os.path.join(SOURCE, name), os.path.join(TARGET, name))
    return True


if __name__ == "__main__":
    print("copied" if sync() else "no protocol/ beside this checkout: nothing copied")
