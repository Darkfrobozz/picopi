# CYW43439 firmware

Every Pico W firmware image embeds the vendor firmware for the CYW43439 radio.
These are the same blobs the C SDK ships as C arrays
(`pico-sdk/lib/cyw43-driver/firmware/*.h`); we take the raw `.bin` form from the
embassy project, which redistributes them for the Rust driver.

| File | What it is |
|---|---|
| `43439A0.bin` | the WLAN firmware image (~231 KB), uploaded into the radio's RAM at boot |
| `43439A0_clm.bin` | the CLM (Country Locale Matrix) regulatory tables (~1 KB) |
| `nvram_rp2040.bin` | board NVRAM / calibration data |

These are licensed under the **Infineon Permissive Binary License**; the text is
in `LICENSE-permissive-binary-license-1.0.txt` in this directory. They are
gitignored (binary vendor blobs, ~235 KB) — `fetch-firmware.sh` is the source of
truth for where they come from.

Fetch them once:

```bash
./fetch-firmware.sh
```

They are gitignored (binary vendor blobs, ~235 KB) — this script is the source
of truth for where they come from.
