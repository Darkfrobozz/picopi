/* Linker memory layout for the RP2040 (Pico / Pico W).
 *
 * Standard embassy-rp layout:
 *   BOOT2  - the second-stage bootloader (256 bytes at the start of flash)
 *   FLASH  - the application
 *   RAM    - all 264K as one block (see OPTION B below to split it)
 */
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100

    /* OPTION A: all RAM banks as one block (used here). */
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K

    /* OPTION B: keep the unstriped sections separate. */
    /* RAM       : ORIGIN = 0x20000000, LENGTH = 256K */
    /* SCRATCH_A : ORIGIN = 0x20040000, LENGTH = 4K   */
    /* SCRATCH_B : ORIGIN = 0x20041000, LENGTH = 4K   */
}
