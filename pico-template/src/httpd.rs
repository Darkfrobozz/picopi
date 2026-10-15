//! A tiny HTTP server — the Rust equivalent of the pico-sdk `httpd.c` example.
//!
//! Serves a page that controls the onboard LED (which lives on the radio chip,
//! so it is driven through [`Radio::turn_led`]).
//!
//!   * `GET /`            -> HTML page with ON/OFF links
//!   * `GET /led/on`      -> turn the onboard LED on
//!   * `GET /led/off`     -> turn it off
//!   * `GET /status`      -> JSON status (also used to discover the device)
//!
//! One connection at a time, `Connection: close`, no keep-alive. That is plenty
//! for a control page and keeps the buffers on the stack.

use core::fmt::Write as _;

use defmt::{info, warn};
use embassy_time::Duration;
use picopi_radio::embassy_net::tcp::{TcpListener, TcpSocket};
use picopi_radio::embedded_io_async::{Read, Write};
use picopi_radio::Radio;

const PORT: u16 = 80;
const REQ_CAP: usize = 1024;
const RESP_CAP: usize = 1024;

/// Serve HTTP forever. Returns `!`, so `main` can end with `httpd::serve(radio).await`.
pub async fn serve(radio: Radio) -> ! {
    let stack = radio.stack();

    let mut listener = match TcpListener::new(stack) {
        Ok(l) => l,
        Err(_) => {
            defmt::panic!("httpd: could not create listener");
        }
    };
    if listener.listen(PORT).is_err() {
        defmt::panic!("httpd: could not bind port {}", PORT);
    }
    info!("httpd: listening on port {}", PORT);

    let mut led_on = false;

    loop {
        let token = match listener.accept().await {
            Ok(token) => token,
            Err(_) => {
                warn!("httpd: accept failed");
                continue;
            }
        };

        // Buffers live for one connection; the socket borrows them and is
        // dropped before the next iteration.
        let mut rx = [0u8; REQ_CAP];
        let mut tx = [0u8; RESP_CAP];
        let mut socket = match TcpSocket::new(stack, &mut rx, &mut tx) {
            Ok(s) => s,
            Err(_) => {
                warn!("httpd: no socket available");
                continue;
            }
        };
        socket.set_timeout(Some(Duration::from_secs(5)));

        if socket.accept(token).await.is_err() {
            warn!("httpd: accept(token) failed");
            continue;
        }

        if let Err(_) = handle(&mut socket, &radio, &mut led_on).await {
            warn!("httpd: io error");
        }
        socket.close();
    }
}

/// Handle exactly one request and write the response.
async fn handle<S>(socket: &mut S, radio: &Radio, led_on: &mut bool) -> Result<(), S::Error>
where
    S: Read + Write,
{
    // --- read request headers (up to CRLFCRLF) ------------------------------
    let mut req = [0u8; REQ_CAP];
    let mut n = 0;
    loop {
        if n == req.len() || headers_complete(&req[..n]) {
            break;
        }
        let got = socket.read(&mut req[n..]).await?;
        if got == 0 {
            break; // peer closed
        }
        n += got;
    }

    // Request line: "GET /path HTTP/1.1"
    let line_end = find(&req[..n], b"\r\n").unwrap_or(n);
    let line = core::str::from_utf8(&req[..line_end]).unwrap_or("");
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");

    info!("httpd: {} {}", method, path);

    // --- route --------------------------------------------------------------
    let mut body = [0u8; 512];
    let mut b = Buf::new(&mut body);
    let (status, ctype) = match (method, path) {
        ("GET", "/") => {
            write_page(&mut b, *led_on);
            ("200 OK", "text/html; charset=utf-8")
        }
        ("GET", "/led/on") => {
            radio.turn_led(true).await;
            *led_on = true;
            write_page(&mut b, true);
            ("200 OK", "text/html; charset=utf-8")
        }
        ("GET", "/led/off") => {
            radio.turn_led(false).await;
            *led_on = false;
            write_page(&mut b, false);
            ("200 OK", "text/html; charset=utf-8")
        }
        ("GET", "/status") => {
            let _ = write!(
                b,
                "{{\"device\":\"pico-light\",\"led\":{},\"gpio\":\"onboard\"}}\n",
                *led_on
            );
            ("200 OK", "application/json")
        }
        _ => {
            let _ = b.write_str("not found\n");
            ("404 Not Found", "text/plain; charset=utf-8")
        }
    };
    let body = b.as_str();

    // --- response -----------------------------------------------------------
    let mut resp = [0u8; RESP_CAP];
    let mut w = Buf::new(&mut resp);
    let _ = write!(
        w,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {ctype}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    socket.write_all(w.as_slice()).await?;
    socket.flush().await
}

fn write_page(b: &mut Buf, led_on: bool) {
    let _ = write!(
        b,
        "<!doctype html><meta charset=utf-8><title>pico-light</title>\
         <style>body{{font-family:sans-serif;margin:2rem}}\
         a{{display:inline-block;padding:.4rem .8rem;margin:.2rem;border:1px solid #888;\
         border-radius:.3rem;text-decoration:none;color:#036}}</style>\
         <h1>pico-light</h1>\
         <p>Onboard LED: <b>{}</b></p>\
         <p><a href=\"/led/on\">ON</a><a href=\"/led/off\">OFF</a>\
         <a href=\"/status\">status</a></p>",
        if led_on { "ON" } else { "OFF" }
    );
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

fn headers_complete(buf: &[u8]) -> bool {
    find(buf, b"\r\n\r\n").is_some()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// A `core::fmt::Write` sink over a fixed buffer.
struct Buf<'a> {
    data: &'a mut [u8],
    len: usize,
}

impl<'a> Buf<'a> {
    fn new(data: &'a mut [u8]) -> Self {
        Self { data, len: 0 }
    }
    fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_slice()).unwrap_or("")
    }
}

impl core::fmt::Write for Buf<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        if end > self.data.len() {
            return Err(core::fmt::Error);
        }
        self.data[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
