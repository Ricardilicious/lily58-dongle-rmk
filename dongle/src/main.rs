//! Lily58 Pro Wireless dongle (nice!nano / nRF52840).
//!
//! A USB receiver for the keyboard: it pairs with the left half over BLE and
//! relays its key reports and Vial to the PC. It has no keymap of its own.
//! It also drives the 1.3" SH1106 OLED with the keyboard state the left half
//! streams to it (layer, WPM, battery of both halves).

#![no_std]
#![no_main]

mod renderer;

use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::mode::Async;
use embassy_nrf::peripherals::{RNG, TWISPI0, USBD};
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{bind_interrupts, rng, twim, usb};
use embassy_time::Duration;
use nrf_mpsl::Flash;
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use rmk::config::{DeviceConfig, StorageConfig};
use rmk::display::DisplayProcessor;
use rmk::dongle::{Dongle, DongleRouter};
use rmk::storage::new_storage_without_keymap;
use rmk::usb::UsbTransport;
use rmk::{DefaultPacketPool, PacketPool, run_all};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<USBD>;
    CLOCK_POWER => nrf_sdc::mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RNG => rng::InterruptHandler<RNG>;
    EGU0_SWI0 => nrf_sdc::mpsl::LowPrioInterruptHandler;
    RADIO => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TIMER0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RTC0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TWISPI0 => twim::InterruptHandler<TWISPI0>;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

const L2CAP_TXQ: u8 = 4;
const L2CAP_RXQ: u8 = 4;

/// Central-only controller: the dongle scans for and connects to the keyboard,
/// it never advertises.
fn build_sdc<'d, const N: usize>(
    p: nrf_sdc::Peripherals<'d>,
    rng: &'d mut rng::Rng<Async>,
    mpsl: &'d MultiprotocolServiceLayer,
    mem: &'d mut sdc::Mem<N>,
) -> Result<nrf_sdc::SoftdeviceController<'d>, nrf_sdc::Error> {
    sdc::Builder::new()?
        .support_scan()
        .support_central()
        .support_dle_central()
        .support_phy_update_central()
        .support_le_2m_phy()
        // One keyboard, one link.
        .central_count(1)?
        .buffer_cfg(
            DefaultPacketPool::MTU as u16,
            DefaultPacketPool::MTU as u16,
            L2CAP_TXQ,
            L2CAP_RXQ,
        )?
        .build(p, rng, mpsl, mem)
}

fn ble_addr() -> [u8; 6] {
    let ficr = embassy_nrf::pac::FICR;
    let high = u64::from(ficr.deviceid(1).read());
    let addr = high << 32 | u64::from(ficr.deviceid(0).read());
    let addr = addr | 0x0000_c000_0000_0000;
    unwrap!(addr.to_le_bytes()[..6].try_into())
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Lily58 Pro Wireless dongle");

    let mut nrf_config = embassy_nrf::config::Config::default();
    // nice!nano v2 has the DCDC inductors fitted
    nrf_config.dcdc.reg0_voltage = Some(embassy_nrf::config::Reg0Voltage::_3V3);
    nrf_config.dcdc.reg0 = true;
    nrf_config.dcdc.reg1 = true;
    let p = embassy_nrf::init(nrf_config);

    // BLE controller
    let mpsl_p = mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
        rc_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
        rc_temp_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
        accuracy_ppm: 500,
        skip_wait_lfclk_started: mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
    };
    static MPSL: StaticCell<MultiprotocolServiceLayer> = StaticCell::new();
    static SESSION_MEM: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(unwrap!(mpsl::MultiprotocolServiceLayer::with_timeslots(
        mpsl_p,
        Irqs,
        lfclk_cfg,
        SESSION_MEM.init(mpsl::SessionMem::new())
    )));
    spawner.spawn(mpsl_task(&*mpsl).unwrap());
    let sdc_p = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26,
        p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );
    let mut rng = rng::Rng::new(p.RNG, Irqs);
    let mut sdc_mem = sdc::Mem::<6080>::new();
    let sdc = unwrap!(build_sdc(sdc_p, &mut rng, mpsl, &mut sdc_mem));

    // USB to the PC
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));
    let device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Ricardilicious",
        product_name: "Lily58 Pro Wireless",
        ..DeviceConfig::default()
    };

    // Storage only holds the paired keyboard. Same region the halves use
    // (below the UF2 bootloader).
    let storage_config = StorageConfig {
        start_addr: 0xA0000,
        num_sectors: 8,
        clear_storage: cfg!(feature = "reset"),
        ..Default::default()
    };
    let flash = Flash::take(mpsl, p.NVMC);
    let mut storage = new_storage_without_keymap(flash, storage_config).await;

    // 1.3" SH1106 OLED: SDA -> D2 (P0.17), SCL -> D3 (P0.20)
    static DISPLAY_I2C_BUF: StaticCell<[u8; 256]> = StaticCell::new();
    let display_i2c = twim::Twim::new(
        p.TWISPI0,
        Irqs,
        p.P0_17,
        p.P0_20,
        twim::Config::default(),
        DISPLAY_I2C_BUF.init([0u8; 256]),
    );
    let display_interface = rmk::display::display_interface_i2c::I2CInterface::new(display_i2c, 0x3C, 0x40);
    let display: rmk::display::oled_async::mode::graphics::GraphicsMode<_, _> =
        rmk::display::oled_async::Builder::new(rmk::display::oled_async::displays::sh1106::Sh1106_128_64 {})
            .with_rotation(rmk::display::oled_async::displayrotation::DisplayRotation::Rotate0)
            .connect(display_interface)
            .into();
    let mut display_processor = DisplayProcessor::with_renderer(display, renderer::DongleRenderer::default())
        // Redraw at least every 0.5 s so the startup screen goes away on its own
        .with_render_interval(Duration::from_millis(500))
        .with_min_render_interval(Duration::from_millis(50));

    // Shared by the BLE side (keyboard) and the USB side (PC)
    let router = DongleRouter::new();
    let mut dongle = Dongle::new(sdc, ble_addr(), &router);
    let mut usb_transport = UsbTransport::new(driver, device_config).with_dongle_router(&router);

    run_all!(usb_transport, dongle, storage, display_processor).await;
}
