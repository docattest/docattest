"""Build the wheels of the ``docattest`` distribution: one per platform, each carrying the
docattest core for that platform as ``docattest/bin/docattest``.

    python3 adapters/python/build_wheel.py --out <directory> \\
        --core darwin-arm64=core/target/release/docattest [--core <platform>=<binary> ...]

The Python code is pure, so a wheel is ``py3-none-<platform>``; the tags come from
``protocol/platforms.json`` (one Linux wheel serves glibc and musl, because the core is a
static binary). Building takes two steps, so that the package's metadata has one source,
``pyproject.toml``:

1. setuptools builds the package once, from a copy of this directory and of ``protocol/``
   (nothing is written into the checkout), into a wheel without a core, which is never
   published (``pip wheel``; it fetches setuptools for the build, unless ``--no-isolation``
   says to use the one installed);
2. each platform's wheel is that wheel, plus the binary with its executable bit, with the
   platform's tags in ``WHEEL``, ``RECORD`` written again, and the platform in its name.

A binary built for another platform than the one it is given for is refused. Only the
standard library is used here, and the package itself needs nothing at run time.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zipfile
from typing import cast

from docattest.launcher import Platform, Platforms

HERE = os.path.dirname(os.path.abspath(__file__))
REPOSITORY = os.path.normpath(os.path.join(HERE, "..", ".."))
EXECUTABLE = 0o100755 << 16


def platforms() -> dict[str, Platform]:
    with open(os.path.join(REPOSITORY, "protocol", "platforms.json"), encoding="utf-8") as f:
        return cast(Platforms, json.load(f))["platforms"]


def built_for(path: str) -> tuple[str, str | None] | None:
    """``(os, cpu)`` in npm's names, from the binary's header; None when it is not one."""
    with open(path, "rb") as f:
        head = f.read(4096)
    machines = {
        0x3E: "x64",
        0xB7: "arm64",
        0x8664: "x64",
        0xAA64: "arm64",
        0x01000007: "x64",
        0x0100000C: "arm64",
    }
    if head[:4] == b"\x7fELF":
        return "linux", machines.get(struct.unpack_from("<H", head, 18)[0])
    if struct.unpack_from("<I", head, 0)[0] == 0xFEEDFACF:
        return "darwin", machines.get(struct.unpack_from("<I", head, 4)[0])
    if head[:2] == b"MZ":
        pe = struct.unpack_from("<I", head, 0x3C)[0]
        if head[pe : pe + 4] == b"PE\0\0":
            return "win32", machines.get(struct.unpack_from("<H", head, pe + 4)[0])
    return None


def stage(into: str) -> str:
    """A copy of the package and of the protocol, laid out as in the repository."""
    shutil.copytree(
        os.path.join(REPOSITORY, "protocol"),
        os.path.join(into, "protocol"),
        ignore=shutil.ignore_patterns("conformance"),
    )
    package = os.path.join(into, "adapters", "python")
    ignore = shutil.ignore_patterns(
        "__pycache__", "tests", "bin", "build", "dist", "*.egg-info", "build_wheel.py", "conformance.json"
    )
    shutil.copytree(HERE, package, ignore=ignore)
    shutil.copyfile(os.path.join(REPOSITORY, "LICENSE"), os.path.join(package, "LICENSE"))
    return package


def build_base(package: str, into: str, isolation: bool) -> str:
    """The pure wheel setuptools builds, without a core."""
    command = [sys.executable, "-m", "pip", "wheel", "--no-deps", "--wheel-dir", into, package]
    subprocess.run(command + ([] if isolation else ["--no-build-isolation"]), check=True)
    (wheel,) = [name for name in os.listdir(into) if name.endswith("-py3-none-any.whl")]
    return os.path.join(into, wheel)


def record_line(name: str, data: bytes) -> str:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
    return f"{name},sha256={digest},{len(data)}"


def platform_wheel(base: str, core: str, entry: Platform, out: str) -> str:
    """The base wheel with the core in it, tagged for one platform."""
    tags = [f"py3-none-{tag}" for tag in entry["wheel"]]
    name, version = os.path.basename(base).split("-")[:2]
    target = os.path.join(out, f"{name}-{version}-py3-none-{'.'.join(entry['wheel'])}.whl")
    dist_info = f"{name}-{version}.dist-info"
    with open(core, "rb") as f:
        binary = f.read()
    records: list[str] = []
    with zipfile.ZipFile(base) as source, zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as wheel:
        stamp = source.getinfo(f"{dist_info}/METADATA").date_time

        def add(info: zipfile.ZipInfo, data: bytes) -> None:
            wheel.writestr(info, data, zipfile.ZIP_DEFLATED)
            records.append(record_line(info.filename, data))

        for info in source.infolist():
            if info.filename in (f"{dist_info}/RECORD", f"{dist_info}/WHEEL"):
                continue
            add(info, source.read(info))
        bin_info = zipfile.ZipInfo(f"docattest/bin/{entry['binary']}", stamp)
        bin_info.external_attr = EXECUTABLE
        add(bin_info, binary)
        lines = ["Wheel-Version: 1.0", "Generator: docattest build_wheel.py", "Root-Is-Purelib: false"]
        add(
            zipfile.ZipInfo(f"{dist_info}/WHEEL", stamp),
            ("\n".join(lines + [f"Tag: {t}" for t in tags]) + "\n").encode(),
        )
        records.append(f"{dist_info}/RECORD,,")
        wheel.writestr(zipfile.ZipInfo(f"{dist_info}/RECORD", stamp), "\n".join(records) + "\n")
    return target


def verify(path: str, entry: Platform) -> None:
    """The wheel holds the core, executable, exactly once, and says the platform's tags."""
    with zipfile.ZipFile(path) as wheel:
        names = wheel.namelist()
        cores = [n for n in names if n.startswith("docattest/bin/")]
        assert cores == [f"docattest/bin/{entry['binary']}"], cores
        assert (wheel.getinfo(cores[0]).external_attr >> 16) & 0o111 == 0o111, "not executable"
        (wheel_file,) = [n for n in names if n.endswith(".dist-info/WHEEL")]
        tags = [line[5:] for line in wheel.read(wheel_file).decode().splitlines() if line.startswith("Tag: ")]
        assert tags == [f"py3-none-{t}" for t in entry["wheel"]], tags
        assert wheel.testzip() is None


def main() -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    parser.add_argument("--out", required=True, help="the directory the wheels are written to")
    parser.add_argument("--core", action="append", required=True, metavar="PLATFORM=BINARY")
    parser.add_argument(
        "--no-isolation", action="store_true", help="build with the setuptools installed here"
    )
    args = parser.parse_args()
    known = platforms()
    pairs: list[str] = args.core
    cores = dict(pair.split("=", 1) for pair in pairs)
    for platform, core in cores.items():
        if platform not in known:
            sys.exit(f"unknown platform {platform!r}; known: {', '.join(known)}")
        entry = known[platform]
        if built_for(core) != (entry["os"], entry["cpu"]):
            sys.exit(
                f"{core} is not a core for {platform} ({entry['os']} {entry['cpu']}): it is {built_for(core)}"
            )
    os.makedirs(args.out, exist_ok=True)
    with tempfile.TemporaryDirectory() as temp:
        base = build_base(stage(temp), os.path.join(temp, "base"), not args.no_isolation)
        for platform, core in cores.items():
            out: str = args.out
            wheel = platform_wheel(base, core, known[platform], out)
            verify(wheel, known[platform])
            print(wheel)


if __name__ == "__main__":
    main()
