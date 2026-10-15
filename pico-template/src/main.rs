//! pico-template — bare-bones starting point for a Raspberry Pi Pico W project.
//!
//! Does the minimum: bring up USB (so flashing needs no BOOTSEL button), bring
//! up the radio with [`Radio::init`], join WiFi, and blink the onboard LED.
//!
//! Everything radio-related lives in the `picopi-radio` crate; see its
//! `examples/` directory for richer usage (HTTP server, external LED, …).

#![no_std]
#![no_main]

mod credentials;
mod usb;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_time::Timer;
use picopi_radio::{radio_pins, Config, Radio};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // USB CDC device: implements the 1200-baud BOOTSEL reset so flashing needs
    // no BOOTSEL button.
    usb::init(&spawner, p.USB);

    // One call: firmware upload, driver/net/actor tasks, DHCP.
    let radio = unwrap!(Radio::init(
        spawner,
        radio_pins!(p),
        Config::new(credentials!(wifi_ssid), credentials!(wifi_password)),
    )
    .await);

    info!("wifi: joining \"{}\"", credentials!(wifi_ssid));
    match radio.join().await {
        Ok(()) => {
            // Only wait for DHCP after a successful association, otherwise
            // wait_link_up() would block forever.
            radio.wait_up().await;
            info!("network: up");
        }
        Err(e) => defmt::warn!("wifi: join failed: {:?}", e),
    }

    let mut on = false;
    loop {
        on = !on;
        radio.turn_led(on).await;
        Timer::after_millis(500).await;
    }
}
