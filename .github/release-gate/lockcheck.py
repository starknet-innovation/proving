"""Checks a release-gate Cargo.lock against the lockfile it was seeded from.

usage: lockcheck.py <seed.lock> <Cargo.lock> <expected privacy-prove source>

Fails if a crates.io package resolved to a version the seed does not pin, so the gate runs the
same third-party code as the build it vouches for, or if privacy-prove did not resolve to the
expected source, so the candidate's [patch] took effect and the reference has none.
"""

import sys
import tomllib


def packages(path):
    with open(path, "rb") as f:
        return tomllib.load(f).get("package", [])


def is_registry(package):
    return package.get("source", "").startswith(("registry+", "sparse+"))


def main():
    seed_path, lock_path, expected_source = sys.argv[1:]
    seed = {(p["name"], p["version"]) for p in packages(seed_path) if is_registry(p)}
    lock = packages(lock_path)

    errors = []
    drift = sorted(
        f"{p['name']} {p['version']}"
        for p in lock
        if is_registry(p) and (p["name"], p["version"]) not in seed
    )
    if drift:
        errors.append("crates.io packages the seed lockfile does not pin:\n  " + "\n  ".join(drift))
    sources = sorted(p.get("source", "<path>") for p in lock if p["name"] == "privacy-prove")
    if sources != [expected_source]:
        errors.append(f"privacy-prove resolved to {sources}, expected [{expected_source}]")

    if errors:
        sys.exit(f"{lock_path}:\n" + "\n".join(errors))
    print(f"{lock_path}: privacy-prove from {expected_source}; no crates.io drift from the seed")


if __name__ == "__main__":
    main()
