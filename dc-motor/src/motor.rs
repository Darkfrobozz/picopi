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

// Both channel-A and channel-B constructors are part of the skeleton's API;
// only the one the project uses is actually called.
#![allow(dead_code)]

use embassy_rp::Peri;
use embassy_rp::pwm::{ChannelAPin, ChannelBPin, Config, Pwm, Slice};

/// PWM period in counts. `clk_sys` 125 MHz / (divider 1 × 6250) = 20 kHz.
const TOP: u16 = 6249;

#[derive(Clone, Copy)]
enum Chan {
    A,
    B,
}

pub struct Motor<'d> {
    pwm: Pwm<'d>,
    cfg: Config,
    chan: Chan,
}

impl<'d> Motor<'d> {
    /// Motor on PWM **channel A** — even GPIOs.
    ///
    /// On the RP2040, GPIO `n` belongs to PWM slice `(n / 2) % 8`; even `n` is
    /// channel A, odd `n` is channel B. GP16 = slice 0 A, GP14 = slice 7 A.
    pub fn new<T: Slice>(slice: Peri<'d, T>, pin: Peri<'d, impl ChannelAPin<T>>) -> Self {
        let mut cfg = Config::default();
        cfg.top = TOP;
        let pwm = Pwm::new_output_a(slice, pin, cfg.clone());
        Self {
            pwm,
            cfg,
            chan: Chan::A,
        }
    }

    /// Motor on PWM **channel B** — odd GPIOs. GP15 = slice 7 B, GP17 = slice 0 B.
    pub fn new_b<T: Slice>(slice: Peri<'d, T>, pin: Peri<'d, impl ChannelBPin<T>>) -> Self {
        let mut cfg = Config::default();
        cfg.top = TOP;
        let pwm = Pwm::new_output_b(slice, pin, cfg.clone());
        Self {
            pwm,
            cfg,
            chan: Chan::B,
        }
    }

    /// Set throttle, `0.0..=1.0` (clamped).
    pub fn set(&mut self, speed: f32) {
        let speed = speed.clamp(0.0, 1.0);
        let compare = (speed * (TOP as f32 + 1.0)) as u16;
        match self.chan {
            Chan::A => self.cfg.compare_a = compare,
            Chan::B => self.cfg.compare_b = compare,
        }
        self.pwm.set_config(&self.cfg);
    }

    /// Stop the motor (0 % duty).
    pub fn stop(&mut self) {
        self.set(0.0);
    }
}
