//! Dongle status screen for a 128x64 SH1106 OLED.
//!
//! Port of the ZMK `custom_status_screen.c` layout:
//!
//! ```text
//!   LILY58                 42 WPM
//!   ------------------------------
//!                BASE
//!   ------------------------------
//!   L [################----]  85%
//!   R [###########---------]  62%
//! ```
//!
//! The screen is blanked while the keyboard sleeps to avoid OLED burn-in.

use core::fmt::Write as _;

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_10X20};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle, RoundedRectangle};
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::event::BatteryStatusEvent;
use rmk::types::battery::{BatteryStatus, ChargeState};

const W: i32 = 128;

const LAYER_NAMES: [&str; 4] = ["BASE", "LOWER", "RAISE", "ADJUST"];

#[derive(Default)]
pub struct DongleRenderer;

impl DisplayRenderer<BinaryColor> for DongleRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if ctx.sleeping {
            return;
        }

        let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let big = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let line = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

        // Header: name (or CAPS) on the left, WPM on the right
        let title = if ctx.caps_lock { "CAPS" } else { "LILY58" };
        draw_text(display, title, 0, 0, small, Alignment::Left);
        let mut wpm: heapless::String<12> = heapless::String::new();
        write!(wpm, "{} WPM", ctx.wpm).ok();
        draw_text(display, &wpm, W - 1, 0, small, Alignment::Right);

        Line::new(Point::new(0, 11), Point::new(W - 1, 11))
            .into_styled(line)
            .draw(display)
            .ok();

        // Layer name in the middle
        let layer = ctx.layer as usize;
        let mut name: heapless::String<8> = heapless::String::new();
        match LAYER_NAMES.get(layer) {
            Some(n) => name.push_str(n).ok(),
            None => write!(name, "L{}", layer).ok(),
        };
        draw_text(display, &name, W / 2, 13, big, Alignment::Center);

        Line::new(Point::new(0, 35), Point::new(W - 1, 35))
            .into_styled(line)
            .draw(display)
            .ok();

        // Battery rows for the two halves (peripheral 0 = left, 1 = right)
        for (i, label) in ["L", "R"].iter().enumerate() {
            let connected = ctx.peripherals_connected.get(i).copied().unwrap_or(false);
            let battery = ctx
                .peripheral_batteries
                .get(i)
                .copied()
                .unwrap_or(BatteryStatusEvent(BatteryStatus::Unavailable));
            draw_battery_row(display, label, connected, battery, 40 + i as i32 * 13);
        }
    }
}

fn draw_text<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    text: &str,
    x: i32,
    y: i32,
    style: MonoTextStyle<'_, BinaryColor>,
    alignment: Alignment,
) {
    let text_style = TextStyleBuilder::new()
        .alignment(alignment)
        .baseline(Baseline::Top)
        .build();
    Text::with_text_style(text, Point::new(x, y), style, text_style)
        .draw(display)
        .ok();
}

/// `L [=========     ]  85%`
fn draw_battery_row<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    label: &str,
    connected: bool,
    battery: BatteryStatusEvent,
    y: i32,
) {
    const BAR_X: i32 = 10;
    const BAR_W: u32 = 82;
    const BAR_H: u32 = 10;

    let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    draw_text(display, label, 0, y, small, Alignment::Left);

    // Pill-shaped outline
    RoundedRectangle::with_equal_corners(
        Rectangle::new(Point::new(BAR_X, y), Size::new(BAR_W, BAR_H)),
        Size::new(4, 4),
    )
    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
    .draw(display)
    .ok();

    let level = match *battery {
        BatteryStatus::Available { level: Some(pct), .. } => Some(pct.min(100)),
        _ => None,
    };

    if let (true, Some(pct)) = (connected, level) {
        let inner_w = BAR_W - 4;
        let fill = (inner_w * pct as u32 + 50) / 100;
        if fill > 0 {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(BAR_X + 2, y + 2), Size::new(fill, BAR_H - 4)),
                Size::new(2, 2),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .ok();
        }
    }

    let mut text: heapless::String<6> = heapless::String::new();
    if !connected {
        text.push_str("OFF").ok();
    } else {
        match *battery {
            BatteryStatus::Available {
                charge_state: ChargeState::Charging,
                ..
            } => text.push_str("CHG").ok(),
            BatteryStatus::Available { level: Some(pct), .. } => write!(text, "{}%", pct.min(100)).ok(),
            _ => text.push_str("--").ok(),
        };
    }
    draw_text(display, &text, W - 1, y + 1, small, Alignment::Right);
}
