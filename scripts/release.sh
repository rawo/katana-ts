#!/usr/bin/env bash
# Starts a release: sets the version in every crate and the PKGBUILD, commits,
# tags vVERSION and pushes. GitHub Actions (.github/workflows/release.yml) then
# builds the pacman package and publishes it with SLSA provenance.
#
#   scripts/release.sh 0.2.0
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: scripts/release.sh X.Y.Z   (e.g. 0.2.0)" >&2
  exit 2
fi
tag="v$version"
cd "$(git rev-parse --show-toplevel)"

[[ "$(git branch --show-current)" == main ]] || { echo "switch to main first" >&2; exit 1; }
[[ -z "$(git status --porcelain --untracked-files=no)" ]] || { echo "commit or stash your changes first" >&2; exit 1; }
git fetch --quiet origin main --tags
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || { echo "main isn't the same as origin/main; pull or push first" >&2; exit 1; }
! git rev-parse -q --verify "refs/tags/$tag" >/dev/null || { echo "tag $tag exists already" >&2; exit 1; }

for f in katana-proto/Cargo.toml katana-cli/Cargo.toml katana-gui/Cargo.toml; do
  # Only the [package] version: the first `version = ` line of each manifest.
  awk -v v="$version" '!done && /^version = "/ { $0 = "version = \"" v "\""; done = 1 } { print }' "$f" > "$f.tmp"
  mv "$f.tmp" "$f"
done
sed -i.bak "s/^pkgver=.*/pkgver=$version/" packaging/arch/PKGBUILD && rm packaging/arch/PKGBUILD.bak
cargo update --workspace --quiet   # the crates' own versions in Cargo.lock

git add katana-proto/Cargo.toml katana-cli/Cargo.toml katana-gui/Cargo.toml Cargo.lock packaging/arch/PKGBUILD
git commit --quiet -m "release $tag"
git tag -a "$tag" -m "katana-ts $tag"
git push --quiet origin main "$tag"
echo "pushed $tag; the build: https://github.com/rawo/katana-ts/actions/workflows/release.yml"
