//! dc-motor — Pico W project skeleton for driving brushed DC motors.
//!
//! * **GP15** — PWM motor output (`src/motor.rs`), drives a low-side N-MOSFET.
//!   Runs a slow spin-up / spin-down bench test.
//! * **GP16** — plain GPIO status LED (optional; move or remove freely).
//! * **onboard LED** — lives on the radio chip, driven via `Radio::turn_led`.
//!
//! GP15 is PWM slice 7, channel B (odd GPIO), hence `Motor::new_b`.

#![no_std]
#![no_main]

mod credentials;
mod motor;
mod usb;

use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};
use motor::Motor;
use picopi_radio::{Radio, radio_pins};
use {defmt_rtt as _, panic_probe as _};

/// Half-period of the LED blinks.
const BLINK: Duration = Duration::from_millis(500);

/// Blinks an external LED on GP15 (plain GPIO, no radio involvement).
#[embassy_executor::task]
async fn lamp(mut led: Output<'static>) -> ! {
    loop {
        led.toggle();
        Timer::after(BLINK).await;
    }
}

/// Bench test: ramp the motor up and down so you can watch the PWM work.
#[embassy_executor::task]
async fn motor_task(mut motor: Motor<'static>) -> ! {
    // Let the board settle before anything spins.
    Timer::after_secs(3).await;
    loop {
        for step in 0..=100u32 {
            motor.set(step as f32 / 100.0);
            Timer::after_millis(20).await;
        }
        Timer::after_millis(500).await;
        for step in (0..=100u32).rev() {
            motor.set(step as f32 / 100.0);
            Timer::after_millis(20).await;
        }
        motor.stop();
        Timer::after_millis(1500).await;
    }
}

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // USB CDC device: implements the 1200-baud BOOTSEL reset so flashing needs
    // no BOOTSEL button.
    usb::init(&spawner, p.USB);

    // Status LED on GP16.
    spawner.spawn(unwrap!(lamp(Output::new(p.PIN_16, Level::Low))));

    // Motor output on GP15 (PWM slice 7, channel B).
    spawner.spawn(unwrap!(motor_task(Motor::new_b(p.PWM_SLICE7, p.PIN_15))));

    // Radio bring-up (firmware upload, driver/net/actor tasks). No join().
    let radio = unwrap!(Radio::init(
        spawner,
        radio_pins!(p),
        picopi_radio::Config::new(credentials!(wifi_ssid), credentials!(wifi_password)),
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
