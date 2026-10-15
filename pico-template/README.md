# pico-template

Baseline Rust firmware for the **Raspberry Pi Pico W** (RP2040), used as the
starting point for projects in this repo.

What it sets up:

- **CYW43439 radio** over gSPI (PIO + DMA) — see the top-level `README` and the
  notes in `FLASHING.md`
- **`embassy-net`** stack with DHCPv4
- **WiFi association** using credentials baked in at build time
- **LED blinks** — GP15 (plain GPIO, plus/minus any external LED you attach) from
  boot, and the onboard LED (`WL_GPIO0` on the radio) once setup is done

## Build & flash

```bash
# one-time: fetch the radio firmware blobs
../firmware/fetch-firmware.sh

# compile, then build + flash over USB (no sudo, no BOOTSEL button)
cargo run --release
```

`cargo run` uses the `runner` in `.cargo/config.toml`:

```toml
runner = ["bash", "flash-runner.sh"]
```

`flash-runner.sh` asks the running firmware for a **1200-baud reset**
(`src/usb.rs` implements that: `stty -F /dev/ttyACM0 1200`), waits for the
`RPI-RP2` volume, loads the ELF with `picotool`, and reboots into the app.

If the device is unreachable (fresh board, hung firmware, USB disabled), the
runner tells you to use the BOOTSEL button. See `../FLASHING.md` for all
fallbacks.

### USB and the 1200-baud reset

The template brings up a CDC-ACM device in `src/usb.rs` whose only job is to
watch the line coding and call `reset_to_usb_boot()` on a 1200-baud request.
That is what keeps flashing button-free. Delete `src/usb.rs`, `mod usb;` and the
`usb::init(...)` call if a project doesn't want USB.

## Credentials

Secrets are parsed at **build time** by `build.rs` and compiled into the image;
nothing is read from disk at runtime. Search order (first hit wins):

1. `$PICO_CREDENTIALS` — path to a TOML file
2. `<repo>/secrets/credentials.toml` ← intended location
3. `<project>/secrets/credentials.toml`
4. `~/.config/picopi/credentials.toml`
5. `secrets/credentials.toml.example` — placeholders, with a build warning

Fill it in:

```bash
cp ../secrets/credentials.toml.example ../secrets/credentials.toml
$EDITOR ../secrets/credentials.toml
```

```toml
[wifi]
ssid     = "Beacon"
password = "…"
country  = "SE"
```

`src/credentials.rs` `include!`s the generated module and exposes a
`credentials!` macro:

```rust
use crate::credentials;          // module
let ssid = credentials!(wifi_ssid);
```

To add another secret, add the key to the TOML and emit a matching `const` in
`build.rs`.

## Layout

```
pico-template/
├── Cargo.toml
├── build.rs            linker setup + credentials parsing/generation
├── flash-runner.sh     cargo runner: 1200-baud reset + picotool load
├── memory.x            RP2040 flash/RAM layout
├── rust-toolchain.toml pins stable + thumbv6m-none-eabi
├── .cargo/config.toml  target + flash runner + DEFMT_LOG
├── README.md
└── src/
    ├── main.rs         tasks: cyw43 runner, net runner, lamp, main
    ├── usb.rs          USB CDC that implements the 1200-baud BOOTSEL reset
    └── credentials.rs  include!(generated) + credentials! macro
```

## Starting a new project

```bash
cp -r pico-template my-project
# edit my-project/Cargo.toml  (name = "my-project")
cd my-project
cargo run --release
```

If you nest the project deeper, adjust the `aligned_bytes!("../../firmware/…")`
paths in `src/main.rs` accordingly.
