//! Left half firmware (first `[[split.peripheral]]` in keyboard.toml).
#![no_main]
#![no_std]

use rmk::macros::rmk_peripheral;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {}
