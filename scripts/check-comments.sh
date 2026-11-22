#!/usr/bin/env bash
set -euo pipefail

roots=("$@")
if [ "${#roots[@]}" -eq 0 ]; then
  roots=(src tests fixtures)
fi

status=0
while IFS= read -r -d '' file; do
  stripped="$(sed -E -e 's/"([^"\\]|\\.)*"//g' -e "s/'([^'\\\\]|\\\\.)*'//g" "$file")"
  if printf '%s\n' "$stripped" | rg -n '//|/\*'; then
    echo "comments found in $file" >&2
    status=1
  fi
done < <(find "${roots[@]}" -type f \( -name '*.rs' -o -name '*.c' \) -print0)
exit "$status"
