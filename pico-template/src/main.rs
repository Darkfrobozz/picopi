//! pico-template — baseline Rust firmware for the Raspberry Pi Pico W (RP2040).
//!
//! What it brings up:
//!
//! * the CYW43439 radio, over gSPI bit-banged with PIO + DMA
//! * an `embassy-net` stack with DHCPv4
//! * a WiFi association using credentials baked in at build time
//! * a blink on **GP15** (a plain GPIO, independent of the radio) and a blink
//!   on the **onboard LED** (which lives on the radio chip, `WL_GPIO0`)
//!
//! Extend from here: spawn more tasks, add a TCP/HTTP server, sensors, etc.

#![no_std]
#![no_main]

mod credentials;
mod usb;

use cyw43::{JoinOptions, aligned_bytes};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::{info, unwrap, warn};
use embassy_executor::Spawner;
use embassy_net::{StackStorage, StaticPool};
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

/// Half-period of the LED blink.
const BLINK: Duration = Duration::from_millis(500);

/// How many times to attempt the WiFi association before giving up.
const WIFI_ATTEMPTS: u32 = 3;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

/// Owns the CYW43439 and drives its state machine forever.
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>, cyw43::Cyw43439>,
) -> ! {
    runner.run().await
}

/// Runs the network stack (DHCP, TCP/IP timers, ...).
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static>) -> ! {
    runner.run().await
}

/// Blinks an external LED on GP15. Plain GPIO, so it starts instantly and does
/// not depend on the radio or on WiFi association.
#[embassy_executor::task]
async fn lamp_task(mut led: Output<'static>) -> ! {
    loop {
        led.toggle();
        Timer::after(BLINK).await;
    }
}

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut rng = RoscRng;

    // USB CDC device: implements the 1200-baud BOOTSEL reset so flashing needs
    // no BOOTSEL button. Bring it up first so it is available immediately.
    usb::init(&spawner, p.USB);

    // The external lamp starts blinking immediately, before any radio bring-up.
    spawner.spawn(unwrap!(lamp_task(Output::new(p.PIN_15, Level::Low))));

    // CYW43439 firmware, embedded at compile time (run firmware/fetch-firmware.sh).
    let fw = aligned_bytes!("../../firmware/43439A0.bin");
    let clm = aligned_bytes!("../../firmware/43439A0_clm.bin");
    let nvram = aligned_bytes!("../../firmware/nvram_rp2040.bin");

    // gSPI link to the radio: GP23 power, GP25 chip-select, GP24 data, GP29 clock.
    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        dma::Channel::new(p.DMA_CH0, Irqs),
        dma::Channel::new(p.DMA_CH1, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let (net_device, mut control, runner) =
        cyw43::new(STATE.init(cyw43::State::new()), pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    // --- network stack ----------------------------------------------------
    let seed = rng.next_u64();

    static STACK: StaticCell<StackStorage> = StaticCell::new();
    static POOL: StaticPool<{ cyw43::RX_BUF_SIZE }, 16, { cyw43::RX_BUF_ALIGN }> = StaticPool::new();
    let (stack, runner) = embassy_net::Stack::new(STACK.init(StackStorage::new()), &POOL, seed);

    static DEVICE: StaticCell<cyw43::NetDriver<'static>> = StaticCell::new();
    let iface = unwrap!(stack.add_iface_borrowed(DEVICE.init(net_device)));
    unwrap!(iface.set_dhcpv4(Some(Default::default())));
    spawner.spawn(unwrap!(net_task(runner)));

    // --- WiFi (best effort: never block the blink forever) ----------------
    info!(
        "wifi: joining \"{}\" (credentials from {})",
        credentials::WIFI_SSID,
        credentials::SOURCE
    );

    let mut connected = false;
    for attempt in 1..=WIFI_ATTEMPTS {
        match control
            .join(
                credentials::WIFI_SSID,
                JoinOptions::new(credentials::WIFI_PASSWORD.as_bytes()),
            )
            .await
        {
            Ok(()) => {
                info!("wifi: associated");
                connected = true;
                break;
            }
            Err(e) => {
                warn!("wifi: attempt {}/{} failed: {:?}", attempt, WIFI_ATTEMPTS, e);
                Timer::after_secs(2).await;
            }
        }
    }

    if connected {
        info!("wifi: waiting for link and DHCP");
        iface.wait_link_up().await;
        iface.wait_config_up().await;
        // NOTE: to log the address, read it from the interface config API
        // (current embassy-net exposes `is_config_v4_up()`; the address getter
        // has moved around between releases).
        info!("network: up");
    } else {
        warn!("wifi: not connected, continuing without network");
    }

    // --- onboard LED blink, forever --------------------------------------
    // (The onboard LED is WL_GPIO0 on the radio, hence driven via `control`.)
    let mut on = false;
    loop {
        on = !on;
        control.gpio_set(0, on).await;
        Timer::after(BLINK).await;
    }
}
