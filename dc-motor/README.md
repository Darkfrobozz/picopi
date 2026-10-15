# dc-motor

Pico W skeleton for driving **brushed DC motors** — the 8520 "coreless" motors
used in micro drones, and similar.

| Pin | Used for |
|---|---|
| **GP16** | PWM motor output → gate of a low-side N-MOSFET (spin-up/down bench test) |
| **GP15** | plain GPIO, blinks from boot (status LED) |
| — | onboard LED, on the radio chip, via `Radio::turn_led` |

WiFi is not joined; the radio is only brought up because the onboard LED needs
it. Change the motor pin in `src/main.rs` if your gate is elsewhere.

## Motor wiring (per motor)

```
  +V (motor rail) ──┬──────────────┐
                    │              │
                  [motor]        [Schottky]   flyback, cathode to +V
                    │              │
                    └──────┬───────┘
                           │ drain
             GP16 ── gate  MOSFET (logic-level, ≥5 A, low R_DS(on))
                           │ source
                          GND
```

Notes:

- Use a **logic-level N-MOSFET** (AO3400, IRLML2502, …). A BJT will drop voltage
  and run hot.
- **Flyback diode across the motor**, not across the FET.
- **100 nF ceramic across the motor terminals** and a **100–470 µF bulk cap** on
  the motor rail. Brushed motors are electrically noisy and can reset the Pico.
- **Separate motor supply**, common ground. Do not run motors from the Pico 3V3.
- 8520 stall current is ~2–4 A per motor; the supply and FET must handle it.

## Software

`src/motor.rs` is a small PWM driver:

```rust
let mut motor = Motor::new(p.PWM_SLICE0, p.PIN_16);   // GP16 = slice 0, chan A
motor.set(0.5);        // 50 % throttle, 0.0..=1.0
motor.stop();
```

PWM is **20 kHz** (`clk_sys 125 MHz / 6250`, `top = 6249`), above hearing and
smooth for a brushed motor.

GPIO → slice mapping on the RP2040: slice `(n / 2) % 8`, channel A for even `n`,
B for odd. GP17 = slice 0 B, GP14 = slice 7 A, GP15 = slice 7 B, and so on.

`main.rs` runs a slow spin-up/spin-down bench test 3 s after boot so you can
watch PWM work before wiring anything clever.

## Build & flash

```bash
cargo run --release          # build + flash over USB, no BOOTSEL button

# props/load off the first time — the bench test spins the motor
```

Credentials are wired in (the radio needs a `Config`) but unused until you add
`radio.join()`. See `../pico-template/README.md` for the credential setup.

## Next

- **Four motors**: one slice per motor, then a mixer
  (`throttle + roll/pitch/yaw → 4 duty values`).
- **Control loop**: IMU over SPI + PID, plus a **failsafe that forces 0 %** on
  signal loss.
- **Closed loop**: brushed motors have no RPM feedback the way bidirectional
  DShot does; add a hall sensor or optical tacho if you need it.
