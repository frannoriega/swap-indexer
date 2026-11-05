#!/usr/bin/env bash
# Usage:
#   scripts/mod_cycles.sh              # check every lib and bin target in the workspace
#   scripts/mod_cycles.sh <package>    # check all lib/bin targets of one package
# Exits 1 if any target has a module cycle.
set -uo pipefail
here="$(dirname "$0")"

# Prints "<package> <lib|bin> <target-name>" per line, optionally filtered by package.
targets() {
  cargo metadata --no-deps --format-version 1 | python3 -c '
import json, sys
only = sys.argv[1] if len(sys.argv) > 1 else None
m = json.load(sys.stdin)
ws = set(m["workspace_members"])
for p in m["packages"]:
    if p["id"] not in ws or (only and p["name"] != only):
        continue
    for t in p["targets"]:
        if "bin" in t["kind"]:
            print(p["name"], "bin", t["name"])
        elif any(k in ("lib", "rlib") for k in t["kind"]):
            print(p["name"], "lib", t["name"])
' "$@"
}

check() { # package kind target
  local pkg=$1 kind=$2 name=$3 flag=--lib
  [[ $kind == bin ]] && flag="--bin $name"
  echo "== $pkg ($kind${kind:+ }$([[ $kind == bin ]] && echo "$name"))"
  # shellcheck disable=SC2086
  cargo modules dependencies -p "$pkg" $flag --no-externs --no-sysroot --no-owns 2>/dev/null \
    | python3 "$here/mod_cycles.py"
}

status=0
found=0
while read -r pkg kind name; do
  found=1
  check "$pkg" "$kind" "$name" || status=1
done < <(targets "$@")

if [[ $found -eq 0 ]]; then
  echo "no lib/bin targets found${1:+ for package '$1'}" >&2
  exit 2
fi
exit $status
