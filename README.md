# Lily58 Pro Wireless (nice!nano dongle, RMK + Vial)

- **Dongle** (3rd nice!nano + 1.3" SH1106 OLED) = split central. It runs the keymap and Vial, and plugs into the PC over USB.
- **Left / right halves** = peripherals, connected to the dongle over BLE.
- 56 keys (the inner key under each nice!nano isn't used).

The firmware is built by GitHub Actions. Open the **Actions** tab, open the latest run, and download the `firmware` artifact.

| File | Flash to |
| --- | --- |
| `lily58_dongle.uf2` | dongle |
| `lily58_left.uf2` | left half |
| `lily58_right.uf2` | right half |
| `*_reset.uf2` | clears saved settings (keymap / pairing) for that board |

## First flash (coming from ZMK)

For each board: plug it in, double-tap reset so the `NICENANO` drive appears, then drag the file onto it.

1. Flash `lily58_dongle_reset.uf2`, then `lily58_dongle.uf2` to the dongle.
2. Flash `lily58_left_reset.uf2`, then `lily58_left.uf2` to the left half.
3. Flash `lily58_right_reset.uf2`, then `lily58_right.uf2` to the right half.
4. Unplug the halves and switch them on. They pair with the dongle by themselves, and the OLED shows `L` / `R` with battery levels.

## Bootloader without opening the case

| Board | How |
| --- | --- |
| Dongle | Hold Lower + Raise (Adjust), then press a top corner key |
| Left half | Hold **Esc** (top-left) while switching it on or plugging in USB |
| Right half | Hold **`** (top-right) while switching it on or plugging in USB |

## Adjust layer (hold Lower + Raise)

- Top corners: dongle bootloader
- Key below each corner: reboot the dongle
- Left end of 3rd row: **hold 5 s** to forget the paired halves, then power-cycle everything to re-pair

## Changing the keymap

- **Live:** open [Vial](https://get.vial.today/) with the dongle plugged in. No unlock combo is needed. Changes are saved on the dongle.
- **Default keymap:** edit `keyboard.toml` and push. Vial changes stay saved until you flash a `_reset` file.

## OLED

The screen layout is in `src/renderer.rs`. It shows WPM (and CAPS), the current layer with an icon, and battery bars for both halves. It blanks after 10 minutes idle (`split_central_sleep_timeout_seconds`), which also puts the halves on slower BLE intervals to save battery.

OLED wiring on the dongle: GND→GND, VCC→VCC, SDA→D2 (P0.17), SCL→D3 (P0.20).
