#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Duration, Timer};

use static_cell::StaticCell;

// Program metadata for `picotool info`.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(c"Let's blink and chill."),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

// interrupt types are available via https://docs.embassy.dev/embassy-rp/git/rp2040/interrupt/typelevel/index.html
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // The following GPIO pins are connected to the wireless chip via SPI.
    //
    // cf. https://datasheets.raspberrypi.com/picow/pico-2-w-schematic.pdf
    let pin_wl_on = p.PIN_23;
    let pin_wl_d = p.PIN_24;
    let pin_wl_cs = p.PIN_25;
    let pin_wl_clk = p.PIN_29;

    let outpin_wl_cs = Output::new(pin_wl_cs, Level::High);
    let outpin_wl_pwr = Output::new(pin_wl_on, Level::Low);

    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        outpin_wl_cs,
        pin_wl_d,
        pin_wl_clk,
        p.DMA_CH0,
    );

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());

    // Via this `control`, we can operate the wireless chip at last!
    let (_net_device, mut control, runner) = cyw43::new(state, outpin_wl_pwr, spi, fw).await;

    unwrap!(spawner.spawn(cyw43_task(runner)));

    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");
    control.init(clm).await;

    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let delay = Duration::from_millis(250);
    loop {
        info!("led on!");
        control.gpio_set(0, true).await;
        Timer::after(delay).await;

        info!("led off!");
        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}
