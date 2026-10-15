//! USB CDC-ACM device that implements the "1200-baud touch" BOOTSEL reset.
//!
//! The RP2040's ROM bootloader can only be entered with the BOOTSEL button or
//! by asking for it from running firmware. The Pico SDK's USB stdio watches for
//! a 1200-baud line-coding request and reboots into BOOTSEL; this module does
//! the same, so the button-free flashing workflow keeps working:
//!
//! ```text
//! stty -F /dev/ttyACM0 1200      # -> RPI-RP2 appears
//! ```
//!
//! Drop this module (and `usb::init` in `main`) if a project doesn't need USB.

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::class::cdc_acm::device::{CdcAcmClass, State};
use embassy_usb::{Builder, Config, UsbDevice};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

/// Watches the CDC line coding; a 1200-baud request reboots into BOOTSEL.
#[embassy_executor::task]
async fn reset_task(class: CdcAcmClass<'static, Driver<'static, USB>>) -> ! {
    // Keep the endpoints alive for as long as the device is connected.
    let (_tx, _rx, control) = class.split_with_control();
    loop {
        control.control_changed().await;
        if control.line_coding().data_rate() == 1200 {
            info!("usb: 1200 baud -> rebooting into BOOTSEL");
            reset_to_usb_boot(0, 0);
        }
    }
}

/// Bring up the USB CDC device. Call once, early in `main`.
pub fn init(spawner: &Spawner, usb: Peri<'static, USB>) {
    let driver = Driver::new(usb, Irqs);

    let mut config = Config::new(0x2e8a, 0x000a); // Raspberry Pi Pico (CDC)
    config.manufacturer = Some("picopi");
    config.product = Some("pico-template");
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
        &mut [], // no MS OS descriptors
        CONTROL_BUF.init([0; 64]),
    );

    static STATE: StaticCell<State> = StaticCell::new();
    let class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);

    let device = builder.build();
    spawner.spawn(unwrap!(usb_task(device)));
    spawner.spawn(unwrap!(reset_task(class)));
}
