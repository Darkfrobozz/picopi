# picopi-radio

One-call CYW43439 radio abstraction for the **Raspberry Pi Pico W** (RP2040).

The Pico W's WiFi radio is a separate coprocessor: it must be powered up, have
its firmware uploaded into RAM, and then be serviced forever. This crate hides
all of that — pins, PIO, DMA, IRQ bindings, `StaticCell`s, firmware blobs, the
driver task, the IP stack and the connection pool — behind a single call.

```rust
let radio = Radio::init(
    spawner,
    radio_pins!(p),                         // the Pico W pins/PIO/DMA
    Config::new("Beacon", "secret"),
)
.await?;

radio.turn_led(true).await;                 // onboard LED (on the radio chip)
radio.join().await?;                        // associate
radio.wait_up().await;                      // link + DHCP

let mut conn = radio.connect("1.1.1.1:80".parse().unwrap()).await?;
conn.write_all(b"GET / HTTP/1.0\r\n\r\n").await?;
let n = conn.read(&mut buf).await?;
```

## API

| Method | Plane | Notes |
|---|---|---|
| `Radio::init(spawner, pins, config)` | — | firmware upload, tasks, DHCP |
| `turn_led(bool)` | control | onboard `WL_GPIO0`; serialized |
| `join()` / `leave()` | control | uses the `Config` credentials |
| `rssi()` | control | dBm |
| `connect(SocketAddr) -> Connection` | data | hands out an owned connection |
| `is_link_up()` / `wait_link_up()` / `wait_config_up()` / `wait_up()` | data | link state |
| `stack()` / `iface()` | escape hatch | raw `embassy_net` handles |

`Connection` has `read(&mut [u8])` and `write_all(&[u8])`; it owns a buffer pair
from an internal pool and closes the socket when dropped. `MAX_CONNS`
concurrent connections are supported.

## Design: two planes, one handle

`cyw43` itself separates the two:

- **control** — `cyw43::Control` is `&mut self`, so commands are serialized. A
  dedicated *actor task* owns it; the handle talks to it over a mailbox and
  awaits a reply. That is why `join()` is "a future on a radio request".
- **data** — frames and TCP run on the network stack's own runner task, which is
  independent of the actor. A pending `join()` therefore does **not** stall TCP.

Connections are *handed out* by the control handle but serviced on the data
plane: `connect()` allocates from a static `TcpClientState` pool and returns a
`Connection` — the buffer ownership problem is solved by embassy-net's pool, so
the connection object is self-contained and closes itself on drop.

### Serialization caveat

Because `Control` is `&mut self`, a long `join()` delays other **control**
commands (`turn_led`, `rssi`) — they queue. It does not affect TCP.

If something must animate *during* a join, drive it from a plain GPIO (e.g. an
external LED on GP15) rather than the onboard LED. The template does exactly
this.

## Resources it takes

Fixed by the Pico W wiring: **PIO0**, **DMA CH0 + CH1**, and **GP23/24/25/29**.
`radio_pins!(p)` builds the [`Pins`] struct from your `Peripherals`.

## Firmware blobs

The coprocessor has no flash; its firmware is embedded at compile time from the
repo's shared `firmware/` directory (`43439A0.bin`, `43439A0_clm.bin`,
`nvram_rp2040.bin`). Run `firmware/fetch-firmware.sh` once after cloning — the
crate's `build.rs` fails with a clear message if they are missing.

## HTTP server

`picopi_radio::httpd` is a small async HTTP/1.1 server — the `httpd.c`
equivalent — that controls the onboard LED:

| Path | Effect |
|---|---|
| `GET /` | page with ON/OFF links |
| `GET /led/on` / `/led/off` | onboard LED |
| `GET /status` | JSON, carries `"device":"pico-light"` for LAN discovery |

```rust
radio.join().await?;
radio.wait_up().await;
picopi_radio::httpd::serve(radio).await     // port 80, never returns
```

One connection at a time, `Connection: close`, stack buffers. For concurrency
or keep-alive, use `picoserve` over [`Radio::stack`].

## Examples

`examples/` holds full firmware binaries (they build for `thumbv6m-none-eabi`
and flash like a project):

```bash
cargo run --release --example blink          # onboard LED blink
PICO_WIFI_SSID=Beacon PICO_WIFI_PASSWORD=… \
    cargo run --release --example httpd      # WiFi + HTTP control page
```

`examples/common/mod.rs` carries the shared USB reset (so flashing needs no
BOOTSEL button) and the example credentials (read from the environment at
compile time).

## Tuning

`src/lib.rs` top:

| const | meaning |
|---|---|
| `RX_POOL` | packet buffers in the stack pool |
| `SOCK_TX` / `SOCK_RX` | per-connection buffer sizes |
| `MAX_CONNS` | concurrent TCP connections |
