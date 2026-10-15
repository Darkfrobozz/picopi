//! Example: an HTTP server that controls the onboard LED.
//!
//! Joins WiFi, then serves [`picopi_radio::httpd`] on port 80:
//!
//! * `GET /` — page with ON/OFF links
//! * `GET /led/on` / `GET /led/off`
//! * `GET /status` — JSON (also identifies the device on a LAN)
//!
//! Build/flash with real credentials (read at compile time):
//!
//! ```text
//! PICO_WIFI_SSID=Beacon PICO_WIFI_PASSWORD=… \
//!     cargo run --release --example httpd
//! ```
//!
//! Then find the board and open it, e.g.:
//!
//! ```text
//! curl http://<ip>/status
//! ```

#![no_std]
#![no_main]

mod common;

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use picopi_radio::{Radio, httpd, radio_pins};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    common::usb_reset(&spawner, p.USB);

    let radio = unwrap!(Radio::init(spawner, radio_pins!(p), common::config()).await);

    info!("joining WiFi...");
    match radio.join().await {
        Ok(()) => {
            radio.wait_up().await;
            info!("network up");
        }
        Err(e) => defmt::warn!("join failed: {:?}", e),
    }

    httpd::serve(radio).await
}
