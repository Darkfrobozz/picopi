//! Build script for `picopi-radio`.
//!
//! 1. Make sure the CYW43439 firmware blobs are present (they live in the
//!    repo's shared `firmware/` directory and are gitignored).
//! 2. Set up linking for the crate's `examples/`, which are full firmware
//!    binaries: stage `memory.x` and pass the cortex-m / embassy / defmt linker
//!    arguments.

use std::path::PathBuf;

fn main() {
    firmware();
    example_linker();
}

fn firmware() {
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
}

fn example_linker() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("memory.x"), include_bytes!("memory.x")).expect("write memory.x");

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");

    println!("cargo:rustc-link-arg-examples=--nmagic");
    println!("cargo:rustc-link-arg-examples=-Tlink.x");
    println!("cargo:rustc-link-arg-examples=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-examples=-Tdefmt.x");
}
