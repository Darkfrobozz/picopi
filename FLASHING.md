# Flashing a Pico

Everything here works as your normal user (`frobozz`). **No sudo, no BOOTSEL
button** — as long as the currently-running firmware includes the SDK's USB
stdio stack (see [Caveats](#caveats)).

## TL;DR

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
make -C build -j"$(nproc)"
picotool load -f -v build/<target>.uf2
```

That is the whole loop. `-f` resets the running application into BOOTSEL,
flashes, verifies, and reboots it back into application mode.

## Permissions — why no sudo is needed

`/etc/udev/rules.d/60-pico.rules` tags the Pico's USB devices with
`uaccess`, so systemd-logind grants an ACL to the active session user:

```
# Raspberry Pi Pico family (RP2040 / RP2350) over USB:
SUBSYSTEM=="usb", ATTRS{idVendor}=="2e8a", TAG+="uaccess"
SUBSYSTEM=="block", ATTRS{idVendor}=="2e8a", TAG+="uaccess"
KERNEL=="ttyACM*", ATTRS{idVendor}=="2e8a", TAG+="uaccess"
```

What that buys you:

| Interface | Node | Used for |
|---|---|---|
| USB device (vendor `2e8a`) | `/dev/bus/usb/BBB/DDD` | **picotool** (PICOBOOT / BOOTSEL) |
| CDC serial | `/dev/ttyACM0` | the 1200-baud reset fallback |
| BOOTSEL mass storage | `/dev/sdX1` (`RPI-RP2`) | drag & drop (still needs root to mount — see below) |

Verify it's live:

```bash
getfacl /dev/ttyACM0          # expect: user:frobozz:rw-
ls -l /dev/bus/usb/*/*           # expect a '+' ACL marker on the Pico node
```

If the ACL isn't there, reload the rules and replug:

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## The three ways into BOOTSEL

| Method | Command / action | Needs |
|---|---|---|
| **picotool reset** (best) | `picotool reboot -f -u` | USB stdio in the running firmware |
| **1200-baud touch** | `stty -F /dev/ttyACM0 1200` (~4 s) | USB stdio in the running firmware |
| **BOOTSEL button** (always works) | hold BOOTSEL while plugging USB, or hold BOOTSEL + tap RESET | hands |

`picotool load -f ...` performs the first method for you.

## Flashing methods

### 1. picotool — recommended

```bash
picotool load -f -v build/<target>.uf2     # or .elf
```

- `-f` / `--force`: reset a device that is *not* in BOOTSEL but is running
  "compatible code" (i.e. firmware with the SDK's USB reset interface). After
  the command the device is rebooted back into application mode.
- `-v` / `--verify`: read the flash back and compare.
- Accepts `.uf2`, `.elf`, `.bin` (inferred from the extension, or `-t uf2`).

If the device is *already* in BOOTSEL (button held), drop `-f`:

```bash
picotool load -v build/<target>.uf2
picotool reboot -a            # run it (needed if you didn't use -f/-x)
```

Other useful invocations:

```bash
picotool info -f              # reset running app, read program info, reboot it
picotool load -f -x ...       # -x = execute the loaded image afterwards
picotool load -f -u ...       # -u = only write flash sectors that differ
```

### 2. BOOTSEL mass storage (RPI-RP2) — fallback

Get into BOOTSEL (`picotool reboot -f -u`, or the button), then the drive
appears as `/dev/disk/by-label/RPI-RP2` → `/dev/sda1`.

Mounting is a privileged operation and there is **no** `udisks2` on this
machine, so this path needs root:

```bash
mkdir -p ~/media
sudo mount /dev/sda1 ~/media
sudo cp build/<target>.uf2 ~/media/
sync
sudo umount ~/media
```

(The uaccess tag on the block device is not enough — `mount` needs
`CAP_SYS_ADMIN`. If you want button-free drag & drop without sudo, install
`udisks2` and use `udisksctl mount -b /dev/sda1`.)

Legacy script that does this with sudo: `~/archive/pico-test/flash.sh`.

### 3. 1200-baud touch — soft reset only

Not a flashing method by itself; it just reboots into BOOTSEL:

```bash
stty -F /dev/ttyACM0 1200
# ~4 s later: /dev/disk/by-label/RPI-RP2 appears
```

Works because `pico_stdio_usb` watches for a 1200-baud line-coding request and
calls `rom_reset_usb_boot()`.

## Caveats

Both soft methods (`picotool -f`, 1200-baud) require the running firmware to
include the USB stdio stack with the reset interfaces:

```cmake
pico_enable_stdio_usb(<target> 1)   # and the app must call stdio_init_all()
```

Defaults in SDK 2.2.0 (when not using TinyUSB directly) are:

- `PICO_STDIO_USB_ENABLE_RESET_VIA_BAUD_RATE = 1` (the 1200-baud touch)
- `PICO_STDIO_USB_ENABLE_RESET_VIA_VENDOR_INTERFACE = 1` (picotool's `-f`)

A firmware built with USB stdio **disabled** (e.g. a tiny no-USB blink) exposes
no USB device at all → **only the physical BOOTSEL button** (or SWD via a probe)
can reflash it. That is a deliberate trade in size and startup time.

## Building

Standard pico-sdk CMake project:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
make -C build -j"$(nproc)"
```

Artifacts land in `build/`: `<target>.uf2`, `<target>.elf`, `.bin`, `.hex`,
plus `.dis`/`.map` when configured.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `No accessible RP-series devices in BOOTSEL mode were found` | Device is running an app, not in BOOTSEL. Use `picotool load -f ...` or `picotool reboot -f -u`. |
| `picotool -f` says it can't reset the device | The running firmware has no USB reset interface (USB stdio disabled). Use the BOOTSEL button. |
| `/dev/ttyACM0` missing | Running firmware has no USB CDC (or the board hung). Use the BOOTSEL button. |
| `picotool: permission denied` | udev ACL missing — reload rules, replug, check `getfacl`. |
| Board re-enumerates as nothing after flashing | Normal for a no-USB firmware. The LED should still run. |
| Flashing a project in `~/Pico` after moving it | Delete the stale `build/` and reconfigure — CMake caches absolute paths. |
