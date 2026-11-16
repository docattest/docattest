# pyright: reportMissingModuleSource=false
# setuptools is the build backend: pip installs it for the build, not into the lint tools.
"""Build hook: the package ships the protocol's schemas and the pinned core version, copied
from the repository's ``protocol/`` directory each time it is built from a checkout, and
PyPI's page shows the README with links that lead somewhere."""

import json
import os
import re
import sys

from setuptools import setup
from setuptools.command.build_py import build_py

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from sync_protocol import TARGET, sync  # noqa: E402

REPOSITORY = "https://github.com/docattest/docattest"
# Where this package sits in the repository, which its README's relative links start from.
PACKAGE_PATH = "adapters/python"
# A link's target in Markdown, `](target)`, when it is relative: no scheme, no leading `/`
# and not an anchor of the page itself.
RELATIVE_LINK = re.compile(r"\]\((?![A-Za-z][A-Za-z0-9+.-]*:|/|#)([^)\s]+)\)")


class BuildWithProtocol(build_py):
    def run(self) -> None:
        sync()
        super().run()


def readme_for_pypi() -> str:
    """The README, with each relative link made absolute: PyPI shows it on a page of its own,
    where a link relative to this directory leads nowhere. The links lead to the files as
    they were at the release's tag, so the page describes the version it is for."""
    sync()
    with open(os.path.join(TARGET, "core.json"), encoding="utf-8") as pin:
        version = json.load(pin)["version"]
    with open(os.path.join(HERE, "README.md"), encoding="utf-8") as readme:
        text = readme.read()
    base = f"{REPOSITORY}/blob/v{version}/"
    return RELATIVE_LINK.sub(lambda link: f"]({base}{repository_path(link.group(1))})", text)


def repository_path(relative: str) -> str:
    """Where a link relative to this package's directory leads, from the repository's root."""
    path, anchor, fragment = relative.partition("#")
    return os.path.normpath(os.path.join(PACKAGE_PATH, path)).replace(os.sep, "/") + anchor + fragment


setup(
    cmdclass={"build_py": BuildWithProtocol},
    long_description=readme_for_pypi(),
    long_description_content_type="text/markdown",
    project_urls={"Source": REPOSITORY, "Issues": f"{REPOSITORY}/issues"},
)
