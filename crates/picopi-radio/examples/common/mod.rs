//! Shared helpers for the `picopi-radio` examples.
//!
//! Not a binary itself — `examples/*.rs` do `mod common;`.
#![allow(dead_code)]

use embassy_executor::Spawner;
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::class::cdc_acm::device::{CdcAcmClass, State};
use embassy_usb::{Builder, Config as UsbConfig, UsbDevice};
use picopi_radio::Config;
use static_cell::StaticCell;

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

/// Watches the CDC line coding; a 1200-baud open reboots into BOOTSEL.
#[embassy_executor::task]
async fn reset_task(class: CdcAcmClass<'static, Driver<'static, USB>>) -> ! {
    let (_tx, _rx, control) = class.split_with_control();
    loop {
        control.control_changed().await;
        if control.line_coding().data_rate() == 1200 {
            defmt::info!("usb: 1200 baud -> rebooting into BOOTSEL");
            reset_to_usb_boot(0, 0);
        }
    }
}

/// Bring up a USB CDC device that implements the 1200-baud BOOTSEL reset, so an
/// example can be flashed without the BOOTSEL button.
pub fn usb_reset(spawner: &Spawner, usb: Peri<'static, USB>) {
    let driver = Driver::new(usb, UsbIrqs);

    let mut config = UsbConfig::new(0x2e8a, 0x000a); // Raspberry Pi Pico (CDC)
    config.manufacturer = Some("picopi");
    config.product = Some("picopi-radio example");
    config.serial_number = Some("picopi");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    static STATE: StaticCell<State> = StaticCell::new();
    let class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);

    let device = builder.build();
    spawner.spawn(defmt::unwrap!(usb_task(device)));
    spawner.spawn(defmt::unwrap!(reset_task(class)));
}

/// Credentials for the examples.
///
/// Read at compile time from `PICO_WIFI_SSID` / `PICO_WIFI_PASSWORD` if set;
/// otherwise falls back to placeholders. To run the `httpd` example against a
/// real network:
///
/// ```text
/// PICO_WIFI_SSID=Beacon PICO_WIFI_PASSWORD=… cargo run --example httpd
/// ```
pub fn config() -> Config {
    Config::new(
        option_env!("PICO_WIFI_SSID").unwrap_or("Beacon"),
        option_env!("PICO_WIFI_PASSWORD").unwrap_or("change-me"),
    )
}
