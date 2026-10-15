//! `picopi-radio` — one-call radio setup for the Raspberry Pi Pico W.
//!
//! The Pico W's WiFi radio is a separate coprocessor (CYW43439) that must be
//! powered up, have its firmware uploaded, and then be serviced forever. This
//! crate hides all of that behind a single [`Radio::init`] call and exposes a
//! tiny handle:
//!
//! ```ignore
//! let radio = Radio::init(spawner, radio_pins!(p), Config {
//!     ssid: "Beacon",
//!     password: "…",
//! }).await?;
//!
//! radio.turn_led(true).await;          // onboard LED
//! radio.join().await?;                 // associate
//! radio.wait_up().await;               // link + DHCP
//!
//! let mut conn = radio.connect("1.1.1.1:80".parse().unwrap()).await?;
//! conn.write_all(b"GET / HTTP/1.0\r\n\r\n").await?;
//! ```
//!
//! ## What it owns
//!
//! `init` consumes the radio's pins/PIO/DMA, the driver state, the net-stack
//! buffers and the connection pool — all as crate-private statics. The caller
//! never writes a `StaticCell`, never sees `embedded_bytes!`/PIO/DMA, and never
//! spawns a runner task.
//!
//! ## Two planes
//!
//! * **Control** ([`Radio::join`], [`Radio::turn_led`], [`Radio::rssi`],
//!   [`Radio::leave`]) goes through an actor task that owns `cyw43::Control`.
//!   `Control` is `&mut self`, so these are serialized: a long `join` delays
//!   other radio commands (but not TCP — see below).
//! * **Data** ([`Radio::connect`], and the raw [`Radio::stack`] escape hatch) is
//!   the IP stack, serviced by its own runner task. It is unaffected by the
//!   actor's serialization. Connections are handed out from the control handle
//!   as owned [`Connection`] objects.
//!
//! ## Firmware blobs
//!
//! The CYW43439 has no flash; its firmware (`43439A0.bin`, `43439A0_clm.bin`,
//! `nvram_rp2040.bin`) is embedded at compile time from the repo's shared
//! `firmware/` directory. Run `firmware/fetch-firmware.sh` once after cloning.

#![no_std]

use core::net::SocketAddr;

use cyw43::JoinOptions;
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use embassy_executor::Spawner;
use embassy_net::tcp::client::{TcpClient, TcpClientState, TcpConnection};
use embassy_net::iface::Iface;
use embassy_net::{Stack, StackStorage, StaticPool};
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::dma;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;

/// Escape hatch: the raw `cyw43` driver, for anything not wrapped here.
pub use cyw43;
/// Escape hatch: the raw network stack types.
pub use embassy_net;

/// Number of packet buffers in the stack's RX pool.
const RX_POOL: usize = 16;
/// Per-connection TCP buffer sizes.
const SOCK_TX: usize = 1024;
const SOCK_RX: usize = 1024;
/// Maximum concurrent TCP connections handed out by [`Radio::connect`].
const MAX_CONNS: usize = 2;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

/// The Pico W pins/PIO/DMA the radio needs. [`radio_pins!`] builds this for you.
///
/// These are the fixed Pico W wiring: GP23 power, GP25 chip-select, GP24 data,
/// GP29 clock, on PIO0 with two DMA channels.
pub struct Pins {
    pub pio: Peri<'static, PIO0>,
    pub dma_tx: Peri<'static, DMA_CH0>,
    pub dma_rx: Peri<'static, DMA_CH1>,
    pub power: Peri<'static, PIN_23>,
    pub cs: Peri<'static, PIN_25>,
    pub data: Peri<'static, PIN_24>,
    pub clk: Peri<'static, PIN_29>,
}

/// Build [`Pins`] from an `embassy_rp::Peripherals` value: `radio_pins!(p)`.
///
/// `$p` must be a variable (it is used several times).
#[macro_export]
macro_rules! radio_pins {
    ($p:ident) => {
        $crate::Pins {
            pio: $p.PIO0,
            dma_tx: $p.DMA_CH0,
            dma_rx: $p.DMA_CH1,
            power: $p.PIN_23,
            cs: $p.PIN_25,
            data: $p.PIN_24,
            clk: $p.PIN_29,
        }
    };
}

/// WiFi credentials. Kept as `&'static str` so they can live in the actor task;
/// the intended source is build-time constants (see the template's
/// `credentials!` macro).
pub struct Config {
    pub ssid: &'static str,
    pub password: &'static str,
}

impl Config {
    pub const fn new(ssid: &'static str, password: &'static str) -> Self {
        Self { ssid, password }
    }
}

/// Errors surfaced by [`Radio`].
#[derive(defmt::Format)]
pub enum Error {
    /// The WiFi association failed.
    Join(cyw43::JoinError),
    /// A TCP operation failed.
    Tcp(embassy_net::tcp::Error),
}

// ---------------------------------------------------------------------------
// Mailbox between the handle and the actor task.
// ---------------------------------------------------------------------------

enum Request {
    Led(bool),
    Join,
    Rssi,
    Leave,
}

static REQ: Channel<CriticalSectionRawMutex, Request, 4> = Channel::new();
static LED_REPLY: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static JOIN_REPLY: Signal<CriticalSectionRawMutex, Result<(), cyw43::JoinError>> = Signal::new();
static RSSI_REPLY: Signal<CriticalSectionRawMutex, i32> = Signal::new();
static LEAVE_REPLY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Serializes callers so the single reply `Signal`s are unambiguous. The actor
/// is serialized anyway, so this costs nothing extra.
static CALL_LOCK: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Internal tasks
// ---------------------------------------------------------------------------

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>, cyw43::Cyw43439>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static>) -> ! {
    runner.run().await
}

/// Owns `cyw43::Control` and executes radio commands one at a time.
#[embassy_executor::task]
async fn radio_task(mut control: cyw43::Control<'static>, config: &'static Config) -> ! {
    loop {
        match REQ.receive().await {
            Request::Led(on) => {
                control.gpio_set(0, on).await;
                LED_REPLY.signal(());
            }
            Request::Join => {
                let result = control
                    .join(config.ssid, JoinOptions::new(config.password.as_bytes()))
                    .await;
                JOIN_REPLY.signal(result);
            }
            Request::Rssi => RSSI_REPLY.signal(control.get_rssi().await),
            Request::Leave => {
                control.leave().await;
                LEAVE_REPLY.signal(());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public handle
// ---------------------------------------------------------------------------

/// Handle to the radio. Cheap to clone and pass around.
#[derive(Clone, Copy)]
pub struct Radio {
    client: &'static TcpClient<'static, MAX_CONNS, SOCK_TX, SOCK_RX>,
    stack: Stack<'static>,
    iface: Iface<'static>,
}

impl Radio {
    /// Bring up the CYW43439 and the network stack.
    ///
    /// Consumes the radio peripherals, uploads the firmware, spawns the driver,
    /// net and radio-actor tasks, and starts DHCP. Does **not** join — call
    /// [`Radio::join`] when you want, so the rest of the program can keep
    /// running (e.g. blink an external LED) meanwhile.
    pub async fn init(spawner: Spawner, pins: Pins, config: Config) -> Result<Self, Error> {
        // Firmware blobs live in the repo's shared firmware/ directory.
        let fw = cyw43::aligned_bytes!("../../../firmware/43439A0.bin");
        let clm = cyw43::aligned_bytes!("../../../firmware/43439A0_clm.bin");
        let nvram = cyw43::aligned_bytes!("../../../firmware/nvram_rp2040.bin");

        // Nonstandard gSPI over PIO + DMA (single bidirectional data line).
        let pwr = Output::new(pins.power, Level::Low);
        let cs = Output::new(pins.cs, Level::High);
        let mut pio = Pio::new(pins.pio, Irqs);
        let spi = PioSpi::new(
            &mut pio.common,
            pio.sm0,
            DEFAULT_CLOCK_DIVIDER,
            pio.irq0,
            cs,
            pins.data,
            pins.clk,
            dma::Channel::new(pins.dma_tx, Irqs),
            dma::Channel::new(pins.dma_rx, Irqs),
        );

        static STATE: StaticCell<cyw43::State> = StaticCell::new();
        let (net_device, mut control, runner) =
            cyw43::new(STATE.init(cyw43::State::new()), pwr, spi, fw, nvram).await;
        spawner.spawn(defmt::unwrap!(cyw43_task(runner)));

        control.init(clm).await;
        control
            .set_power_management(cyw43::PowerManagementMode::PowerSave)
            .await;

        // --- network stack --------------------------------------------------
        let seed = RoscRng.next_u64();

        static STACK_STORAGE: StaticCell<StackStorage> = StaticCell::new();
        static POOL: StaticPool<{ cyw43::RX_BUF_SIZE }, RX_POOL, { cyw43::RX_BUF_ALIGN }> =
            StaticPool::new();
        let (stack, net_runner) =
            embassy_net::Stack::new(STACK_STORAGE.init(StackStorage::new()), &POOL, seed);
        spawner.spawn(defmt::unwrap!(net_task(net_runner)));

        static DEVICE: StaticCell<cyw43::NetDriver<'static>> = StaticCell::new();
        static IFACE: StaticCell<Iface<'static>> = StaticCell::new();
        let iface = IFACE.init(defmt::unwrap!(
            stack.add_iface_borrowed(DEVICE.init(net_device))
        ));
        defmt::unwrap!(iface.set_dhcpv4(Some(Default::default())));

        // --- connection factory (hands out Connection objects) --------------
        static CLIENT_STATE: StaticCell<TcpClientState<MAX_CONNS, SOCK_TX, SOCK_RX>> =
            StaticCell::new();
        let client_state = CLIENT_STATE.init(TcpClientState::new());
        static CLIENT: StaticCell<TcpClient<'static, MAX_CONNS, SOCK_TX, SOCK_RX>> =
            StaticCell::new();
        let client: &'static _ = CLIENT.init(TcpClient::new(stack, client_state));

        // --- radio actor ----------------------------------------------------
        static CONFIG: StaticCell<Config> = StaticCell::new();
        let config: &'static Config = CONFIG.init(config);
        spawner.spawn(defmt::unwrap!(radio_task(control, config)));

        Ok(Self {
            client,
            stack,
            iface: *iface,
        })
    }

    // -- control plane (serialized through the actor) -----------------------

    /// Set the onboard LED (`WL_GPIO0` on the radio). Awaits completion.
    pub async fn turn_led(&self, on: bool) {
        let _guard = CALL_LOCK.lock().await;
        REQ.send(Request::Led(on)).await;
        LED_REPLY.wait().await;
    }

    /// Associate with the WiFi network in [`Config`].
    pub async fn join(&self) -> Result<(), Error> {
        let _guard = CALL_LOCK.lock().await;
        REQ.send(Request::Join).await;
        JOIN_REPLY.wait().await.map_err(Error::Join)
    }

    /// Current RSSI in dBm.
    pub async fn rssi(&self) -> i32 {
        let _guard = CALL_LOCK.lock().await;
        REQ.send(Request::Rssi).await;
        RSSI_REPLY.wait().await
    }

    /// Disassociate from the network.
    pub async fn leave(&self) {
        let _guard = CALL_LOCK.lock().await;
        REQ.send(Request::Leave).await;
        LEAVE_REPLY.wait().await;
    }

    // -- link / address (data plane, no mailbox) ----------------------------

    /// Whether the link is up.
    pub fn is_link_up(&self) -> bool {
        self.iface.is_link_up()
    }

    /// Wait until the link is up.
    pub async fn wait_link_up(&self) {
        self.iface.wait_link_up().await
    }

    /// Wait until DHCP has configured the interface.
    pub async fn wait_config_up(&self) {
        self.iface.wait_config_up().await
    }

    /// Wait for link + DHCP. This is what "the network works" means.
    pub async fn wait_up(&self) {
        self.wait_link_up().await;
        self.wait_config_up().await;
    }

    // -- connections --------------------------------------------------------

    /// Open a TCP connection.
    ///
    /// The returned [`Connection`] owns a buffer pair from an internal pool and
    /// closes the socket when dropped. At most [`MAX_CONNS`] exist at once;
    /// further calls fail until one is dropped.
    pub async fn connect(&self, remote: SocketAddr) -> Result<Connection, Error> {
        use embedded_nal_async::TcpConnect;
        let inner = self.client.connect(remote).await.map_err(Error::Tcp)?;
        Ok(Connection { inner })
    }

    // -- escape hatches -----------------------------------------------------

    /// Raw [`embassy_net::Stack`], for listeners/servers and anything not
    /// wrapped here. `Stack` is `Copy`.
    pub fn stack(&self) -> Stack<'static> {
        self.stack
    }

    /// Raw [`embassy_net::Iface`] (link state, addresses, DHCP control).
    pub fn iface(&self) -> Iface<'static> {
        self.iface
    }
}

/// A TCP connection handed out by [`Radio::connect`].
///
/// Reads and writes are async; dropping the value closes the socket and returns
/// its buffers to the pool.
pub struct Connection {
    inner: TcpConnection<'static, 'static, MAX_CONNS, SOCK_TX, SOCK_RX>,
}

impl Connection {
    /// Read some bytes; `Ok(0)` means the peer closed the connection.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        use embedded_io_async::Read;
        self.inner.read(buf).await.map_err(Error::Tcp)
    }

    /// Write all bytes.
    pub async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        use embedded_io_async::Write;
        self.inner.write_all(buf).await.map_err(Error::Tcp)
    }
}
