#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repository_root"

command -v gh >/dev/null 2>&1 || {
    echo "GitHub CLI (gh) is required." >&2
    exit 1
}

current_branch=$(git branch --show-current)
if [[ "$current_branch" != "main" ]]; then
    echo "Release must be dispatched from the main branch; current branch: $current_branch" >&2
    exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet || [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
    echo "Release stopped: the working tree is not clean." >&2
    echo "Commit, stash, or remove every local change before releasing so the published code is unambiguous." >&2
    git status --short >&2
    exit 1
fi

git fetch --quiet origin main --tags
local_commit=$(git rev-parse HEAD)
remote_commit=$(git rev-parse origin/main)
if [[ "$local_commit" != "$remote_commit" ]]; then
    echo "Local main and origin/main differ. Commit and push the release preparation first." >&2
    exit 1
fi

version=$(git show HEAD:Cargo.toml | sed -n 's/^version = "\([^"]*\)"/\1/p' | head -n 1)
if [[ -z "$version" ]]; then
    echo "Unable to read the package version from Cargo.toml." >&2
    exit 1
fi

release_tag=${1:-"v${version}"}
if [[ ! "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    echo "Invalid release tag: $release_tag" >&2
    exit 1
fi
if [[ "$release_tag" != "v${version}" ]]; then
    echo "Tag $release_tag does not match Cargo.toml version $version." >&2
    exit 1
fi
if ! git cat-file -e "HEAD:docs/RELEASE_NOTES_${release_tag}.md" 2>/dev/null; then
    echo "Committed release notes are missing: docs/RELEASE_NOTES_${release_tag}.md" >&2
    exit 1
fi
if git ls-remote --exit-code --tags origin "refs/tags/${release_tag}" >/dev/null 2>&1; then
    echo "Remote tag already exists: $release_tag" >&2
    exit 1
fi

gh auth status >/dev/null
gh workflow run instplot-lite.yml --ref main -f "release_tag=${release_tag}"

repository=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
echo
echo "Release ${release_tag} was handed to GitHub Actions."
echo "The terminal can now be closed; no local waiting is required."
echo "Progress: https://github.com/${repository}/actions/workflows/instplot-lite.yml"
