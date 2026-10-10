#!/usr/bin/env sh
# Rebuild review.html in this folder from sample/before and sample/after.
# The "before" version is committed in a throwaway git repository, the
# "after" version laid over it, and the flow page written here.
#
#   ./generate.sh                                     uses `flowlens` on your PATH
#   FLOWLENS=../../target/release/flowlens ./generate.sh
#
# The .png screenshots are taken by hand from the page and are not remade here.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
bin=${FLOWLENS:-flowlens}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cp -R "$here/sample/before/." "$work/"
cd "$work"
git init -q
git add -A
git -c user.name=sample -c user.email=sample@example.com commit -qm "Before the change"
rm -rf src
cp -R "$here/sample/after/." "$work/"

"$bin" . --review "$here/review.html"
echo "review.html written to $here"
