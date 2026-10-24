#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ $# -gt 1 || ( $# -eq 1 && "$1" != --release ) ]]; then
    echo "Usage: bash scripts/check-user-guide-assets.sh [--release]" >&2
    exit 1
fi
manifest=docs/images/user-guide/manifest.json
jq -e '
    .platform == "Linux" and .scale == 1 and .theme == "light"
    and (.source_commit | test("^[a-f0-9]{40}$"))
    and (.app_version | type == "string")
    and (.images | length > 0)
    and ([.images[].path] | length == (unique | length))
    and all(.images[]; (.sha256 | test("^[a-f0-9]{64}$"))
        and (.path | test("^docs/images/user-guide/(fr|en)/[a-z-]+\\.png$")))
' "$manifest" >/dev/null

while IFS=$'\t' read -r hash path; do
    printf '%s  %s\n' "$hash" "$path" | sha256sum --check --status
    signature="$(od -An -tx1 -N8 "$path" | tr -d ' \n')"
    if [[ "$signature" != 89504e470d0a1a0a ]]; then
        echo "Invalid PNG: $path" >&2
        exit 1
    fi
done < <(jq -r '.images[] | [.sha256,.path] | @tsv' "$manifest")

for language in fr en; do
    guide=docs/USER_GUIDE.md
    [[ "$language" == fr ]] || guide=docs/USER_GUIDE.en.md
    references="$(grep -oE 'images/user-guide/[^) ]+' "$guide")"
    while IFS= read -r path; do
        [[ -n "$path" ]] || continue
        if ! jq -e --arg path "docs/$path" 'any(.images[]; .path == $path)' "$manifest" >/dev/null; then
            echo "Untracked guide image: $path" >&2
            exit 1
        fi
        if [[ "$path" != "images/user-guide/$language/"* ]]; then
            echo "Wrong-language screenshot in $guide: $path" >&2
            exit 1
        fi
    done <<< "$references"
done

diff <(jq -r '.images[].path | select(contains("/fr/")) | sub("/fr/";"/LANG/")' "$manifest" | sort) \
    <(jq -r '.images[].path | select(contains("/en/")) | sub("/en/";"/LANG/")' "$manifest" | sort)

if [[ "${1:-}" == --release ]]; then
    version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/heelonvault-app/Cargo.toml | head -1)"
    jq -e --arg version "$version" '.app_version == $version and .source_dirty == false' "$manifest" >/dev/null
    source_commit="$(jq -r .source_commit "$manifest")"
    git cat-file -e "$source_commit^{commit}"
    # The release check requires re-capture after UI, translations or dependency changes.
    git diff --exit-code "$source_commit" -- crates/heelonvault-app/src/ui \
        crates/heelonvault-app/assets crates/heelonvault-core/locales \
        crates/heelonvault-app/Cargo.toml Cargo.lock
    premium_commit="$(jq -r .premium_commit "$manifest")"
    test "$(git -C ../heelonvault-premium rev-parse HEAD)" = "$premium_commit"
fi
echo "User-guide images: hashes, PNG format, language pairing and references verified."
