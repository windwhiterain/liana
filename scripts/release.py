#!/usr/bin/env python3
"""
Release script for Liana.

Usage:
    python scripts/release.py 0.3.0

What it does:
    1. Bumps the version in Cargo.toml
    2. Commits the bump
    3. Tags the commit
    4. Pushes commit + tag to origin

Run this instead of manually tagging — it keeps the tag and crate version in sync.
"""

import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = REPO_ROOT / "Cargo.toml"


def check_clean_tree() -> None:
    """Fail if there are uncommitted changes."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        capture_output=True, text=True, cwd=REPO_ROOT,
    )
    if result.stdout.strip():
        print("❌ Uncommitted changes. Commit or stash them first.")
        sys.exit(1)


def read_current_version() -> str:
    """Extract the current version from Cargo.toml."""
    text = CARGO_TOML.read_text()
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not m:
        print("❌ Could not find version in Cargo.toml")
        sys.exit(1)
    return m.group(1)


def bump_version(new_version: str) -> None:
    """Replace the version line in Cargo.toml."""
    text = CARGO_TOML.read_text()
    updated = re.sub(
        r'^version\s*=\s*"[^"]+"',
        f'version = "{new_version}"',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    CARGO_TOML.write_text(updated)
    print(f"✓ Bumped version to {new_version}")


def run_git(cmd: list[str]) -> None:
    """Run a git command, die on failure."""
    print(f"  $ git {' '.join(cmd)}")
    result = subprocess.run(["git", *cmd], cwd=REPO_ROOT)
    if result.returncode != 0:
        print(f"❌ git {' '.join(cmd)} failed")
        sys.exit(1)


def main() -> None:
    if len(sys.argv) != 2:
        print(f"Usage: python {sys.argv[0]} <version>")
        print(f"  e.g. python {sys.argv[0]} 0.3.0")
        sys.exit(1)

    raw = sys.argv[1].strip()
    new_version = raw.removeprefix("v")  # accept both "0.3.0" and "v0.3.0"
    tag = f"v{new_version}"

    print(f"Preparing release {tag} ...")
    print()

    check_clean_tree()

    current = read_current_version()
    print(f"Current version: {current}")
    print(f"New version:     {new_version}")
    print()

    bump_version(new_version)

    print()
    print("Committing ...")
    run_git(["add", "Cargo.toml", "Cargo.lock"])
    run_git(["commit", "-m", f"release {tag}"])
    run_git(["tag", tag])

    print()
    print("Pushing ...")
    run_git(["push", "origin", "main"])
    run_git(["push", "origin", tag])

    print()
    print(f"✓ Release {tag} pushed. GitHub Actions is building binaries now.")


if __name__ == "__main__":
    main()
