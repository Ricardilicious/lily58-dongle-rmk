//! Left half firmware: split central. Runs the keymap and Vial, and talks to
//! the PC through the dongle, over Bluetooth, or over USB.
#![no_main]
#![no_std]

use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {}
