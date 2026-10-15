# picopi

Development monorepo for Raspberry Pi Pico work (RP2040 / RP2350, Pico and
Pico W).

`Pico/` is the main development folder: documentation plus one folder per
project. New projects start from `pico-template/`.

## Layout

```
.
├── README.md                    this file
├── FLASHING.md                  how to build + flash a Pico (user-level, no sudo)
├── flash.sh                     small flash helper (picotool wrapper)
├── secrets/
│   ├── credentials.toml.example committed template, safe to share
│   └── credentials.toml         real secrets — GITIGNORED, never commit
├── firmware/                    CYW43439 vendor blobs (gitignored) + fetch script
├── crates/
│   └── picopi-radio/            the radio abstraction (actor + httpd + examples)
├── pico-template/               bare-bones Pico W project skeleton
└── dc-motor/                    project skeleton that blinks both LEDs
```

## Flashing

As your normal user — no sudo, no BOOTSEL button:

```bash
picotool load -f -v <firmware.uf2|firmware.elf>
```

Full details, fallbacks and troubleshooting: **[FLASHING.md](FLASHING.md)**.
For Rust projects `cargo run --release` does this too — the cargo `runner` is
configured to call `picotool`.

## Secrets

Real credentials live in `secrets/credentials.toml`, which is **gitignored**:

```bash
cp secrets/credentials.toml.example secrets/credentials.toml
$EDITOR secrets/credentials.toml
```

Each project's `build.rs` parses that file at **build time** and bakes the
values into the firmware, so nothing is read from disk at runtime. Search order
(see `pico-template/README.md` for details):

1. `$PICO_CREDENTIALS` — path to a TOML file
2. `secrets/credentials.toml` — this repo
3. `<project>/secrets/credentials.toml`
4. `~/.config/picopi/credentials.toml`
5. `secrets/credentials.toml.example` — placeholders, with a build warning

## Projects

- **`pico-template/`** — bare-bones skeleton: USB reset, `Radio::init`, join,
onboard-LED blink.
- **`dc-motor/`** — skeleton that blinks GP15 *and* the onboard LED; the
starting point for the motor controller.

Both are just applications on top of **`crates/picopi-radio`**, which does all
the radio work and also provides a small HTTP server. Usage examples live in
that crate:

```bash
cd crates/picopi-radio
cargo run --release --example blink
cargo run --release --example httpd      # needs PICO_WIFI_SSID/PASSWORD
```

See [`crates/picopi-radio/README.md`](crates/picopi-radio/README.md) for the
design (two planes: an actor for radio control, handles for data).

Start a new project by copying the template:

```bash
cp -r pico-template my-project
# rename the package in my-project/Cargo.toml, then:
cd my-project
cargo run --release
```

## Firmware blobs

Every Pico W image embeds the CYW43439 vendor firmware. Fetch it once:

```bash
./firmware/fetch-firmware.sh
```

The `.bin` files are gitignored; the script pulls them from the embassy project,
which redistributes the same blobs the C SDK ships as C arrays.

## Environment

| Thing | Where / version |
|---|---|
| Pico SDK | `~/.local/share/pico-sdk` (2.2.0) |
| Cross toolchain | `arm-none-eabi-gcc` |
| picotool | `~/.local/bin/picotool` — v2.2.0, built with USB |
| udev rule | `/etc/udev/rules.d/60-pico.rules` — `uaccess`, so no sudo |
| Rust target | `thumbv6m-none-eabi` (installed automatically by `rust-toolchain.toml`) |
| Board | Pico W (RP2040), USB serial `E66440A2CB075A2A` |
