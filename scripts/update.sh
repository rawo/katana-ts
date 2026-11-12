#!/usr/bin/env bash
# Installs or updates katana-ts on the Linux machine (CachyOS / Arch) from the
# newest GitHub release, or from the release given (scripts/update.sh v0.2.0).
# Needs the GitHub CLI, logged in (`gh auth login`): the repository is private.
# Checks the package's SLSA provenance first when slsa-verifier is installed.
set -euo pipefail

repo=rawo/katana-ts
tag="${1:-$(gh release view --repo "$repo" --json tagName -q .tagName)}"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT

gh release download "$tag" --repo "$repo" --dir "$dir" --pattern '*.pkg.tar.zst' --pattern '*.intoto.jsonl'
pkg=$(ls "$dir"/*.pkg.tar.zst)

if command -v slsa-verifier >/dev/null; then
  slsa-verifier verify-artifact "$pkg" --provenance-path "$dir"/*.intoto.jsonl \
    --source-uri "github.com/$repo" --source-tag "$tag"
else
  echo "slsa-verifier not installed: provenance not checked"
fi

installed=$(pacman -Q katana-ts 2>/dev/null || echo "katana-ts: not installed")
echo "$installed -> $(basename "$pkg")"
sudo pacman -U --needed "$pkg"
