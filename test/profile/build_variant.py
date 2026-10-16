#!/usr/bin/env python3
"""Build historical profiling variants from their recorded baseline."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("variant", choices=["baseline", "reserve", "fused", "combined", "stages"])
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    destination = root / "target/entity-profile/variants" / args.variant
    destination.mkdir(parents=True, exist_ok=False)
    baseline = "fe524f7"
    names = subprocess.check_output(
        ["git", "ls-tree", "-r", "--name-only", baseline, "--",
         "Cargo.toml", "Cargo.lock", "src", "tests", "test", "examples"], cwd=root, text=True,
    ).splitlines()
    for name in names:
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(subprocess.check_output(["git", "show", f"{baseline}:{name}"], cwd=root))
    shutil.copytree(root / "test/profile", destination / "test/profile", ignore=shutil.ignore_patterns("__pycache__"))
    with (destination / "Cargo.toml").open("a") as manifest:
        for name, path in [("entity_profile", "entity_profile.rs"), ("allocation_profile", "allocations.rs")]:
            manifest.write(f'\n[[bin]]\nname = "{name}"\npath = "test/profile/{path}"\n')
    patches = ([] if args.variant == "baseline" else
               ["reserve", "fused"] if args.variant == "combined" else [args.variant])
    for patch in patches:
        subprocess.run(["patch", "--batch", "-p1", "-i", str(root / "test/profile/patches" / (patch + ".patch"))], cwd=destination, check=True)
    env = os.environ.copy()
    env.setdefault("RUSTUP_HOME", str(root / ".toolchain/rustup"))
    env.setdefault("CARGO_HOME", str(root / ".toolchain/cargo"))
    cargo = str(root / ".toolchain/cargo/bin/cargo")
    for command in [["test", "--offline"], ["build", "--release", "--offline", "--bins"]]:
        subprocess.run([cargo, *command], cwd=destination, env=env, check=True)
    print(destination / "target/release/entity_profile")


if __name__ == "__main__":
    main()
