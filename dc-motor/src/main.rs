//! dc-motor — Pico W project skeleton that blinks both LEDs.
//!
//! * **GP15** — a plain GPIO, driven by its own task. Starts blinking
//!   immediately and is completely independent of the radio.
//! * **onboard LED** — lives on the radio chip, so it is driven through
//!   [`Radio::turn_led`].
//!
//! The radio is brought up with a single `Radio::init` call (see the
//! `picopi-radio` crate). WiFi is *not* joined here — this is just the blinking
//! baseline for a motor project.

#![no_std]
#![no_main]

mod credentials;
mod usb;

use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};
use picopi_radio::{radio_pins, Config, Radio};
use {defmt_rtt as _, panic_probe as _};

/// Half-period of the blink.
const BLINK: Duration = Duration::from_millis(500);

/// Blinks an external LED / lamp on GP15 (plain GPIO, no radio involvement).
#[embassy_executor::task]
async fn lamp(mut led: Output<'static>) -> ! {
    loop {
        led.toggle();
        Timer::after(BLINK).await;
    }
}

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // USB CDC device: implements the 1200-baud BOOTSEL reset so flashing needs
    // no BOOTSEL button.
    usb::init(&spawner, p.USB);

    // GP15 starts blinking right away.
    spawner.spawn(unwrap!(lamp(Output::new(p.PIN_15, Level::Low))));

    // Radio bring-up (firmware upload, driver/net/actor tasks). No join().
    let radio = unwrap!(Radio::init(
        spawner,
        radio_pins!(p),
        Config::new(credentials!(wifi_ssid), credentials!(wifi_password)),
    )
    .await);

    // Onboard LED blinks alongside GP15.
    let mut on = false;
    loop {
        on = !on;
        radio.turn_led(on).await;
        Timer::after(BLINK).await;
    }
}
