//! PWM driver for a brushed DC motor driven by a low-side N-MOSFET.
//!
//! Suitable for 8520 "coreless" motors and similar. Wiring per motor:
//!
//! ```text
//!   +V(motor rail) ──┬──────────────┐
//!                    │              │
//!                  [motor]        [Schottky]   (flyback, cathode to +V)
//!                    │              │
//!                    └──────┬───────┘
//!                           │  drain
//!                GPIO ── gate  MOSFET (logic-level, ≥5 A)
//!                           │  source
//!                          GND
//! ```
//!
//! Drive the gate with 20 kHz PWM; duty maps to average voltage, so `set(0.5)`
//! is roughly half throttle. Add 100 nF across the motor and a bulk cap on the
//! motor rail — brushed motors are electrically noisy.

use embassy_rp::Peri;
use embassy_rp::pwm::{ChannelAPin, Config, Pwm, Slice};

/// PWM period in counts. `clk_sys` 125 MHz / (divider 1 × 6250) = 20 kHz.
const TOP: u16 = 6249;

pub struct Motor<'d> {
    pwm: Pwm<'d>,
    cfg: Config,
}

impl<'d> Motor<'d> {
    /// Create a motor output on `slice`/`pin`.
    ///
    /// On the RP2040, GPIO `n` belongs to PWM slice `(n / 2) % 8`; even `n` is
    /// channel A, odd `n` is channel B. So:
    ///
    /// | GPIO | slice | channel |
    /// |---|---|---|
    /// | GP16 | 0 | A |
    /// | GP17 | 0 | B |
    /// | GP14 | 7 | A |
    /// | GP15 | 7 | B |
    pub fn new<T: Slice>(slice: Peri<'d, T>, pin: Peri<'d, impl ChannelAPin<T>>) -> Self {
        let mut cfg = Config::default();
        cfg.top = TOP;
        cfg.compare_a = 0;
        let pwm = Pwm::new_output_a(slice, pin, cfg.clone());
        Self { pwm, cfg }
    }

    /// Set throttle, `0.0..=1.0` (clamped).
    pub fn set(&mut self, speed: f32) {
        let speed = speed.clamp(0.0, 1.0);
        self.cfg.compare_a = (speed * (TOP as f32 + 1.0)) as u16;
        self.pwm.set_config(&self.cfg);
    }

    /// Stop the motor (0 % duty).
    pub fn stop(&mut self) {
        self.set(0.0);
    }
}
