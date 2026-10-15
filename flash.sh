#!/usr/bin/env bash
#
# flash.sh - build (optional) and flash a Pico over USB as a normal user.
#
#   ./flash.sh <image.uf2|image.elf>       flash a prebuilt image
#   ./flash.sh -b <project-dir> [target]   cmake+make the project, then flash
#   ./flash.sh -n <image>                  device is already in BOOTSEL (no -f)
#
# Requires picotool (2.x, built with USB) and the udev rule
# /etc/udev/rules.d/60-pico.rules so no sudo is needed. See FLASHING.md.

set -euo pipefail

MODE=flash
FORCE=1
PROJECT=""
TARGET=""
IMAGE=""

usage() {
    sed -n '3,10p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        -b|--build)    MODE=build; PROJECT="${2:-}"; shift 2 ;;
        -n|--no-force) FORCE=0; shift ;;
        -h|--help)     usage 0 ;;
        -*)            echo "flash.sh: unknown option: $1" >&2; usage 2 ;;
        *)             if [ "$MODE" = build ] && [ -z "$TARGET" ]; then
                           TARGET="$1"
                       else
                           IMAGE="$1"
                       fi
                       shift ;;
    esac
done

PICOTOOL="${PICOTOOL:-}"
if [ -z "$PICOTOOL" ]; then
    PICOTOOL="$(command -v picotool 2>/dev/null || true)"
fi
if [ -z "$PICOTOOL" ]; then
    PICOTOOL="$HOME/.local/bin/picotool"
fi
[ -x "$PICOTOOL" ] || { echo "flash.sh: picotool not found (set \$PICOTOOL)" >&2; exit 1; }

if [ "$MODE" = build ]; then
    [ -n "$PROJECT" ] || { echo "flash.sh: -b needs a project directory" >&2; exit 2; }
    [ -f "$PROJECT/CMakeLists.txt" ] || { echo "flash.sh: no CMakeLists.txt in $PROJECT" >&2; exit 2; }

    echo "==> cmake -S $PROJECT -B $PROJECT/build -DCMAKE_BUILD_TYPE=Release"
    cmake -S "$PROJECT" -B "$PROJECT/build" -DCMAKE_BUILD_TYPE=Release >/dev/null

    if [ -n "$TARGET" ]; then
        echo "==> make -C $PROJECT/build $TARGET"
        make -C "$PROJECT/build" -j"$(nproc)" "$TARGET"
        IMAGE="$PROJECT/build/$TARGET.uf2"
    else
        echo "==> make -C $PROJECT/build -j$(nproc)"
        make -C "$PROJECT/build" -j"$(nproc)"

        mapfile -t uf2s < <(find "$PROJECT/build" -maxdepth 1 -name '*.uf2' 2>/dev/null | sort)
        if [ "${#uf2s[@]}" -eq 1 ]; then
            IMAGE="${uf2s[0]}"
        else
            echo "flash.sh: expected exactly one .uf2 in $PROJECT/build, pass a target instead." >&2
            echo "available:" >&2
            printf '  %s\n' "${uf2s[@]##*/}" >&2
            exit 2
        fi
    fi
fi

[ -n "$IMAGE" ] || { echo "flash.sh: no image given" >&2; usage 2; }
[ -f "$IMAGE" ] || { echo "flash.sh: no such image: $IMAGE" >&2; exit 2; }

args=(load -v)
[ "$FORCE" = 1 ] && args+=(-f)

echo "==> $PICOTOOL ${args[*]} $IMAGE"
"$PICOTOOL" "${args[@]}" "$IMAGE"
echo "==> done"
