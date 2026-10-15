# dc-motor

Pico W project skeleton that blinks **both** LEDs, as the starting point for a
DC-motor controller.

- **GP15** — a plain GPIO, driven by its own task. Blinks from boot and is
  completely independent of the radio (so it keeps going during a WiFi join).
- **onboard LED** — lives on the radio chip (`WL_GPIO0`), so it is driven
  through `Radio::turn_led()`.

WiFi is *not* joined here; the radio is only brought up because the onboard LED
needs it.

## Build & flash

```bash
cargo run --release          # build + flash over USB, no BOOTSEL button
```

Credentials are still wired in (the radio needs a `Config`), but unused until
you add a `radio.join()` call — see `../pico-template/README.md` for the
credential setup, which this project shares.

## Layout

```
dc-motor/
├── Cargo.toml
├── build.rs            linker setup + credentials parsing
├── flash-runner.sh     cargo runner: 1200-baud reset + picotool load
├── memory.x
├── rust-toolchain.toml
├── .cargo/config.toml
├── README.md
└── src/
    ├── main.rs         GP15 lamp task + onboard LED loop
    ├── usb.rs          1200-baud BOOTSEL reset
    └── credentials.rs  include!(generated) + credentials! macro
```

The radio itself is `../crates/picopi-radio`, shared by every project here.

## Next

Add motor driving next: a PWM channel on a spare GPIO, a `motor.rs` module, and
(when the motor should be network-controllable) a `radio.join()` plus an HTTP
endpoint — see `../crates/picopi-radio/examples/httpd.rs`.
