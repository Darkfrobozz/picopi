#!/usr/bin/env bash
#
# flash-runner.sh - cargo `runner`: flash an ELF onto the Pico W over USB as a
# normal user, without the BOOTSEL button.
#
# Cargo invokes this as:  bash flash-runner.sh <path-to-elf>
#
# Steps:
#   1. If the firmware is running and exposes a CDC port, ask it for a
#      1200-baud reset (src/usb.rs implements this) -> RPI-RP2 appears.
#   2. picotool load the ELF into flash.
#   3. picotool reboot into the application.
#
# If the device is not reachable, it tells you to use the BOOTSEL button.

set -euo pipefail

elf="${1:?usage: flash-runner.sh <elf>}"
picotool="${PICOTOOL:-$(command -v picotool || true)}"
[ -n "$picotool" ] || picotool="$HOME/.local/bin/picotool"
[ -x "$picotool" ] || { echo "flash-runner: picotool not found" >&2; exit 1; }

bootsel() { [ -b /dev/sda1 ]; }

if ! bootsel && [ -c /dev/ttyACM0 ]; then
    echo "==> requesting 1200-baud reset (no button)"
    stty -F /dev/ttyACM0 1200 2>/dev/null || true
    for _ in $(seq 1 20); do bootsel && break; sleep 1; done
fi

if ! bootsel; then
    echo "flash-runner: Pico is not in BOOTSEL mode." >&2
    echo "  - hold BOOTSEL while plugging USB, or" >&2
    echo "  - hold BOOTSEL, tap RESET, release BOOTSEL" >&2
    exit 1
fi

echo "==> picotool load -v -t elf $elf"
"$picotool" load -v -t elf "$elf"
"$picotool" reboot -a >/dev/null 2>&1 || true
echo "==> done"
