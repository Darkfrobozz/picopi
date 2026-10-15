#!/usr/bin/env bash
#
# fetch-firmware.sh - download the CYW43439 vendor firmware blobs.
#
# Source: the embassy project, which redistributes the same firmware the C SDK
# ships as C arrays. Files are gitignored; run this after a fresh clone.

set -euo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
base="https://raw.githubusercontent.com/embassy-rs/embassy/main/cyw43-firmware"
files=(43439A0.bin 43439A0_clm.bin nvram_rp2040.bin)

for f in "${files[@]}"; do
    if [ -s "$dir/$f" ]; then
        printf 'have    %s (%s bytes)\n' "$f" "$(stat -c %s "$dir/$f")"
        continue
    fi
    printf 'fetch   %s ... ' "$f"
    curl -fsSL "$base/$f" -o "$dir/$f"
    printf '%s bytes\n' "$(stat -c %s "$dir/$f")"
done

echo "done -> $dir"
