//! Minimal example: bring up the radio and blink the **onboard** LED.
//!
//! The onboard LED is `WL_GPIO0` on the radio chip, so even blinking it
//! requires the radio to be booted — which is all `Radio::init` does here.
//!
//! Build/flash (button-free via the 1200-baud reset in `common`):
//!
//! ```text
//! cargo run --release --example blink
//! ```

#![no_std]
#![no_main]

mod common;

use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use picopi_radio::{Radio, radio_pins};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    common::usb_reset(&spawner, p.USB);

    let radio = unwrap!(Radio::init(spawner, radio_pins!(p), common::config()).await);

    let mut on = false;
    loop {
        on = !on;
        radio.turn_led(on).await;
        Timer::after(Duration::from_millis(500)).await;
    }
}
