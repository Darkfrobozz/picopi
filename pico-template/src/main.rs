//! pico-template — baseline Rust firmware for the Raspberry Pi Pico W (RP2040).
//!
//! All the radio bring-up (CYW43439 firmware upload, gSPI/PIO/DMA, the WiFi
//! actor, the IP stack, the connection pool) lives in the `picopi-radio` crate.
//! This file is just the application.
//!
//! Two LEDs:
//!   * an external lamp on GP15 — a plain GPIO, blinks from boot and is
//!     completely independent of the radio (so it animates during `join()` too)
//!   * the onboard LED — lives on the radio chip, driven via `radio.turn_led()`

#![no_std]
#![no_main]

mod credentials;
mod usb;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};
use picopi_radio::{radio_pins, Config, Radio};
use {defmt_rtt as _, panic_probe as _};

/// Half-period of the blinks.
const BLINK: Duration = Duration::from_millis(500);

/// Blinks the external lamp on GP15. Plain GPIO: independent of the radio.
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

    // External lamp starts blinking immediately, before any radio bring-up.
    spawner.spawn(unwrap!(lamp(Output::new(p.PIN_15, Level::Low))));

    // One call does: firmware upload, driver + net + actor tasks, DHCP.
    let radio = unwrap!(Radio::init(
        spawner,
        radio_pins!(p),
        Config::new(credentials!(wifi_ssid), credentials!(wifi_password)),
    )
    .await);

    info!("wifi: joining \"{}\"", credentials!(wifi_ssid));
    if let Err(e) = radio.join().await {
        defmt::warn!("wifi: join failed: {:?}", e);
    }
    radio.wait_up().await;
    info!("network: up");

    // Onboard LED blink (goes through the radio actor).
    let mut on = false;
    loop {
        on = !on;
        radio.turn_led(on).await;
        Timer::after(BLINK).await;
    }
}
