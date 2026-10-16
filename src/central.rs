//! Dongle firmware: split central with no keys, USB to the PC, OLED status screen.
#![no_main]
#![no_std]

pub mod renderer;

use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {}
