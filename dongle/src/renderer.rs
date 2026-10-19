//! Dongle OLED (128x64 SH1106).
//!
//! While the keyboard is linked to the dongle:
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
//! When the keyboard isn't using the dongle (it's off, asleep, or on Bluetooth
//! or USB), the middle shows SEARCHING or PAIRING instead of the layer.
//! A "RIC CUSTOM" startup screen shows for the first few seconds after the
//! dongle is plugged in, and the screen is blanked while the keyboard sleeps.

use core::fmt::Write as _;

use embassy_time::{Duration, Instant};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_10X20};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle, RoundedRectangle};
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::event::{BatteryStatusEvent, DongleState};
use rmk::types::battery::{BatteryStatus, ChargeState};

const W: i32 = 128;

/// How long the startup screen stays up after the dongle is plugged in
const SPLASH_TIME: Duration = Duration::from_secs(3);

const LAYER_NAMES: [&str; 4] = ["BASE", "NUMBERS", "NAV", "ADJUST"];

#[derive(Default)]
pub struct DongleRenderer {
    /// Time of the first render, i.e. when the dongle started
    started: Option<Instant>,
}

impl DisplayRenderer<BinaryColor> for DongleRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();

        let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let big = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let line = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

        let started = *self.started.get_or_insert_with(Instant::now);
        if started.elapsed() < SPLASH_TIME {
            draw_splash(display, big, line);
            return;
        }

        let linked = ctx.dongle_state == DongleState::Connected;
        if linked && ctx.sleeping {
            return;
        }

        // Header: name (or CAPS) on the left, WPM on the right
        let title = if ctx.caps_lock { "CAPS" } else { "LILY58" };
        draw_text(display, title, 0, 0, small, Alignment::Left);
        if linked {
            let mut wpm: heapless::String<12> = heapless::String::new();
            write!(wpm, "{} WPM", ctx.wpm).ok();
            draw_text(display, &wpm, W - 1, 0, small, Alignment::Right);
        }

        Line::new(Point::new(0, 11), Point::new(W - 1, 11))
            .into_styled(line)
            .draw(display)
            .ok();

        // Middle: layer name, or the link state when the keyboard isn't on the dongle
        let mut middle: heapless::String<10> = heapless::String::new();
        match ctx.dongle_state {
            DongleState::Connected => match LAYER_NAMES.get(ctx.layer as usize) {
                Some(n) => middle.push_str(n).ok(),
                None => write!(middle, "L{}", ctx.layer).ok(),
            },
            DongleState::Pairing => middle.push_str("PAIRING").ok(),
            DongleState::Searching => middle.push_str("SEARCHING").ok(),
        };
        draw_text(display, &middle, W / 2, 13, big, Alignment::Center);

        Line::new(Point::new(0, 35), Point::new(W - 1, 35))
            .into_styled(line)
            .draw(display)
            .ok();

        // Battery rows: left half (the keyboard's own battery) and right half
        // (peripheral 0, forwarded by the left half)
        let right = ctx
            .peripheral_batteries
            .first()
            .copied()
            .unwrap_or(BatteryStatusEvent(BatteryStatus::Unavailable));
        draw_battery_row(display, "L", linked, ctx.battery, 40);
        draw_battery_row(display, "R", linked, right, 53);
    }
}

/// Startup screen: "RIC CUSTOM" in a frame.
fn draw_splash<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    big: MonoTextStyle<'_, BinaryColor>,
    line: PrimitiveStyle<BinaryColor>,
) {
    RoundedRectangle::with_equal_corners(
        Rectangle::new(Point::new(0, 0), Size::new(W as u32, 64)),
        Size::new(6, 6),
    )
    .into_styled(line)
    .draw(display)
    .ok();
    draw_text(display, "RIC CUSTOM", W / 2, 22, big, Alignment::Center);
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

/// `L [=========     ]  85%`. Shows `--` until the level is known or while
/// the keyboard isn't linked to the dongle.
fn draw_battery_row<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    label: &str,
    linked: bool,
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

    if let (true, Some(pct)) = (linked, level) {
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
    if !linked {
        text.push_str("--").ok();
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
