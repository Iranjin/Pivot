#!/usr/bin/env python3
"""Generate licenses/COPYRIGHTS.txt from the normal dependency graph."""

import json
import re
import subprocess
from pathlib import Path

TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
ROOT = Path(__file__).resolve().parent.parent


def metadata(target):
    output = subprocess.check_output(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ],
        cwd=ROOT,
        text=True,
    )
    return json.loads(output)


def tree_packages(target):
    output = subprocess.check_output(
        [
            "cargo",
            "tree",
            "-e",
            "normal",
            "-f",
            "{p}",
            "--prefix",
            "none",
            "--target",
            target,
        ],
        cwd=ROOT,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    packages = set()
    for line in output.splitlines():
        match = re.match(r"^(.+?) v([^\s]+)(?:\s+\(.*\))?$", line.strip())
        if match and match.group(1) != "pivot":
            packages.add((match.group(1), match.group(2)))
    return packages


def copyright_notices(package):
    root = Path(package["manifest_path"]).parent
    files = []
    for pattern in ("LICENSE*", "COPYING*", "NOTICE*", "UNLICENSE*"):
        files.extend(root.glob(pattern))
    if (root / "LICENSES").is_dir():
        files.extend((root / "LICENSES").rglob("*"))

    pattern = re.compile(
        r"^\s*copyright\s*(?:\(c\)|©)?\s*(\d{4}(?:\s*[-–]\s*\d{4})?)?\s*(.*)$",
        re.IGNORECASE,
    )
    rejected = (
        "license to reproduce",
        "copyright notice",
        "copyright holder",
        "copyright owner",
        "copyright law",
        "copyright interest",
        "copyright and related",
        "copyright protection",
        "copyright license",
        "copyright permission",
        "copyright disclaimer",
        "copyright infringement",
        "permission notice",
        "all rights reserved",
    )

    notices = []
    for file in sorted(set(files)):
        if not file.is_file():
            continue
        try:
            text = file.read_text(errors="ignore")
        except OSError:
            continue
        for line in text.splitlines():
            match = pattern.match(line)
            if not match:
                continue
            remainder = match.group(2).strip()
            lower = line.lower()
            if len(remainder) < 2 or any(word in lower for word in rejected):
                continue
            notice = line.strip()
            if notice not in notices:
                notices.append(notice)
    return notices


def main():
    packages_by_id = {}
    for target in TARGETS:
        for package in metadata(target)["packages"]:
            packages_by_id[package["id"]] = package

    by_name = {}
    for package in packages_by_id.values():
        by_name.setdefault((package["name"], package["version"]), []).append(package)

    selected = {}
    for target in TARGETS:
        for name, version in tree_packages(target):
            for package in by_name.get((name, version), []):
                selected[package["id"]] = package

    packages = sorted(selected.values(), key=lambda p: (p["name"].lower(), p["version"]))

    lines = [
        "Third-Party Component Notices",
        "",
        "This file lists the normal dependencies of the Pivot CLAP binary,",
        "with copyright notices found in their distributed license files.",
        "License texts are included below in the Licenses window.",
        "",
    ]
    for package in packages:
        if package["name"] == "pivot":
            continue
        license = package.get("license") or "unknown"
        notices = copyright_notices(package)
        lines.append(f"{package['name']} {package['version']} — {license}")
        if notices:
            for notice in notices[:6]:
                lines.append(f"  {notice}")
        else:
            authors = ", ".join(package.get("authors") or [])
            if authors:
                lines.append(f"  Authors: {authors}")
            else:
                lines.append("  (no copyright notice in the package)")
        lines.append("")

    output = ROOT / "licenses" / "COPYRIGHTS.txt"
    output.write_text("\n".join(lines).rstrip() + "\n")
    print(f"Wrote {output} ({len(packages)} packages)")


if __name__ == "__main__":
    main()
