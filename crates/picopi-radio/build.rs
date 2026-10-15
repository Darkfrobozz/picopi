//! Build script: make sure the CYW43439 firmware blobs are present.
//!
//! They live in the repo's shared `firmware/` directory and are gitignored, so
//! a fresh clone must run `firmware/fetch-firmware.sh` once. Fail with a clear
//! message instead of a cryptic `include_bytes!` error.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let firmware = manifest.join("../../firmware");

    for name in ["43439A0.bin", "43439A0_clm.bin", "nvram_rp2040.bin"] {
        let path = firmware.join(name);
        if !path.is_file() {
            panic!(
                "picopi-radio: missing firmware blob {}\n\
                 Run {}/fetch-firmware.sh first.",
                path.display(),
                firmware.display()
            );
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }

    println!("cargo:rerun-if-changed=build.rs");
}
