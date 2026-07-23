use std::thread;
use std::time::Duration;

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::text::{Baseline, Text};

use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::AnyIOPin;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::FromValueType;

use esp_idf_svc::hal::{
    gpio::PinDriver,
    spi::{SpiDeviceDriver, SpiDriverConfig, config},
};

use mipidsi::interface::SpiInterface;
use mipidsi::{
    Builder,
    models::{ILI9341Rgb565, Model},
    options::{ColorInversion, ColorOrder, Orientation, Rotation},
};
use static_cell::StaticCell;

fn main() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let spi = peripherals.spi2;

    let dc = PinDriver::output(peripherals.pins.gpio46)?;
    let mut backlight = PinDriver::output(peripherals.pins.gpio45)?;
    let sclk = peripherals.pins.gpio12;
    let sda = peripherals.pins.gpio11; // MOSI
    let cs = peripherals.pins.gpio10;

    let mut delay = Ets;

    let config = config::Config::new()
        .baudrate(40.MHz().into())
        .data_mode(config::MODE_0);

    let device = SpiDeviceDriver::new_single(
        spi,
        sclk,
        sda,
        None::<AnyIOPin>,
        Some(cs),
        &SpiDriverConfig::new(),
        &config,
    )?;

    //XXX see https://github.com/esp-rs/esp-idf-hal/blob/master/examples/spi_st7789.rs

    static STATIC_CELL: StaticCell<[u8; 512]> = StaticCell::new();
    let display_buffer = STATIC_CELL.init([0_u8; 512]);

    let di = SpiInterface::new(device, dc, display_buffer);

    // create driver
    let mut display = Builder::new(ILI9341Rgb565, di)
        .invert_colors(ColorInversion::Inverted)
        .display_size(
            ILI9341Rgb565::FRAMEBUFFER_SIZE.0,
            ILI9341Rgb565::FRAMEBUFFER_SIZE.1,
        )
        .color_order(ColorOrder::Bgr)
        .orientation(
            Orientation::new()
                .rotate(Rotation::Deg270)
                .flip_horizontal(),
        )
        .init(&mut delay)
        .expect("display builder init");

    // turn on the backlight
    backlight.set_high()?;

    display.clear(Rgb565::BLACK).unwrap();
    let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    Text::with_baseline("Hello world!", Point::default(), style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    println!("Text printed!");

    loop {
        thread::sleep(Duration::from_millis(1000));
    }
}
