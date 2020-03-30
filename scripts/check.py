#!/usr/bin/env python3

import subprocess
import sys
from typing import Dict, Set

# { os: (channel, [triple...]) }
OSS = {
    "android": (
        "stable",
        ["aarch64-linux-android", "armv7-linux-androideabi", "i686-linux-android",],
    ),
    "freebsd": ("stable", ["x86_64-unknown-freebsd"]),
    "linux": ("stable", ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]),
    "macos": ("stable", ["x86_64-apple-darwin"]),
    "redox": ("nightly", ["x86_64-unknown-redox"]),
    "netbsd": ("stable", ["x86_64-unknown-netbsd"]),
    "windows": ("stable", ["x86_64-pc-windows-gnu", "x86_64-pc-windows-msvc"]),
}

if sys.platform == "darwin":
    OSS["ios"] = ("stable", ["aarch64-apple-ios", "x86_64-apple-ios",])


def blue(s: str) -> str:
    return "\033[94m" + s + "\033[0m"


def get_installed_targets() -> Dict[str, Set[str]]:
    installed: Dict[str, Set[str]] = {}
    for channel in ["stable", "nightly"]:
        installed[channel] = set()
        command = f"rustup +{channel} target list"
        completed = subprocess.run(command.split(), capture_output=True)
        if completed.returncode != 0:
            print(completed.stderr)
            sys.exit(1)
        for line in completed.stdout.splitlines():
            split = line.split()
            if len(split) == 2 and split[1].decode("utf-8") == "(installed)":
                triple = split[0].decode("utf-8")
                installed[channel].add(triple)
    return installed


def run_command(command: str):
    print(blue(command))
    completed = subprocess.run(command.split())
    if completed.returncode != 0:
        sys.exit(1)


def run_cargo_check(channel: str, triple: str):
    command = f"cargo +{channel} check --target={triple}"
    run_command(command)


def run_rustup_target_install(channel: str, triple: str):
    command = f"rustup +{channel} target install {triple}"
    run_command(command)


def main():
    installed = get_installed_targets()
    if len(sys.argv) == 1:
        for (channel, ts) in OSS.values():
            for t in ts:
                if t not in installed[channel]:
                    run_rustup_target_install(channel, t)
                run_cargo_check(channel, t)
        # check linux without libc
        linux_triples = OSS["linux"][1]
        for triple in linux_triples:
            command = f"cargo check --target={triple} --no-default-features"
            run_command(command)
    else:
        for os in sys.argv[1:]:
            value = OSS.get(os)
            if value is None:
                available = str(list(OSS.keys()))
                print(f"Unrecognized OS '{os}'. Must be one of: {available}.")
                sys.exit(1)
            (channel, ts) = value
            for t in ts:
                if t not in installed[channel]:
                    run_rustup_target_install(channel, t)
                run_cargo_check(channel, t)
        if "linux" in sys.argv[1]:
            # check linux without libc
            linux_triples = OSS["linux"][1]
            for triple in linux_triples:
                command = f"cargo check --target={triple} --no-default-features"
                run_command(command)


if __name__ == "__main__":
    main()
