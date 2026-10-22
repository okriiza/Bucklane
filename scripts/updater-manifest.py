#!/usr/bin/env python3
"""Builds the updater manifest (latest.json) for one GitHub release.

Runs once after every platform build has uploaded its packages and .sig files,
so platforms can never overwrite each other's entries (which happens when each
build job merges latest.json concurrently).

    GH_TOKEN=... python3 scripts/updater-manifest.py OWNER/REPO RELEASE_ID [--output latest.json]

Fails if any expected package or signature is missing, so an incomplete release
is caught before it is published.
"""

import argparse
import json
import os
import sys
import urllib.request
from datetime import datetime, timezone

API = "https://api.github.com"

# Package file suffix → updater platform keys it serves. The generic key
# (e.g. "windows-x86_64") is what older updaters look up first.
PLATFORMS = [
    ("_aarch64.app.tar.gz", ["darwin-aarch64", "darwin-aarch64-app"]),
    ("_x64.app.tar.gz", ["darwin-x86_64", "darwin-x86_64-app"]),
    (".AppImage", ["linux-x86_64", "linux-x86_64-appimage"]),
    (".deb", ["linux-x86_64-deb"]),
    (".rpm", ["linux-x86_64-rpm"]),
    ("-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]),
    (".msi", ["windows-x86_64-msi"]),
]


def request(url, accept="application/vnd.github+json"):
    req = urllib.request.Request(url, headers={"Accept": accept})
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req) as response:
        return response.read()


def build_manifest(repo, release, read_asset):
    assets = {a["name"]: a for a in release["assets"]}
    platforms, missing = {}, []
    for suffix, keys in PLATFORMS:
        package = next((a for n, a in assets.items() if n.endswith(suffix)), None)
        signature = package and assets.get(package["name"] + ".sig")
        if not package or not signature:
            missing.append(suffix)
            continue
        entry = {
            "signature": read_asset(signature).decode().strip(),
            # The asset API URL is stable from draft to published release.
            "url": f"{API}/repos/{repo}/releases/assets/{package['id']}",
        }
        for key in keys:
            platforms[key] = entry
    if missing:
        raise SystemExit(f"Missing packages or signatures for: {', '.join(missing)}")
    return {
        "version": release["tag_name"].removeprefix("v"),
        "notes": f"See https://github.com/{repo}/releases/tag/{release['tag_name']}",
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z"),
        "platforms": platforms,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("repo")
    parser.add_argument("release_id")
    parser.add_argument("--output", default="latest.json")
    args = parser.parse_args()

    release = json.loads(request(f"{API}/repos/{args.repo}/releases/{args.release_id}"))
    manifest = build_manifest(
        args.repo,
        release,
        lambda asset: request(asset["url"], accept="application/octet-stream"),
    )
    with open(args.output, "w") as f:
        json.dump(manifest, f, indent=2)
    print(f"{args.output}: {manifest['version']}, {len(manifest['platforms'])} platform keys")


if __name__ == "__main__":
    sys.exit(main())
