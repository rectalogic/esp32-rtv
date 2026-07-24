use es8311::Es8311;
use esp_idf_svc::hal::delay::{BLOCK, Ets, FreeRtos};
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::i2s::{I2sDriver, I2sTx, config};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::FromValueType;

const SAMPLE_RATE: u32 = 16000;
const MCLK_FREQ: u32 = SAMPLE_RATE * 256; // 4_096_000 Hz

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    let peripherals = Peripherals::take()?;

    // ==========================================
    // 1. PIN CONFIGURATION
    // ==========================================
    // Typical I2C Bus Pins
    let scl = peripherals.pins.gpio15;
    let sda = peripherals.pins.gpio16;

    // Typical I2S Audio Pins
    let mclk = peripherals.pins.gpio4; // ES8311 strongly relies on Master Clock
    let bclk = peripherals.pins.gpio5;
    let ws = peripherals.pins.gpio7;
    let dout = peripherals.pins.gpio8;

    // Optional: Power Amplifier Enable Pin
    let mut pa_pin = PinDriver::output(peripherals.pins.gpio1)?;
    pa_pin.set_high()?; // Keep amplifier quiet during chip initialization

    // ==========================================
    // 2. INITIALIZE & CONFIGURE I2C FOR ES8311
    // ==========================================
    let i2c_config = I2cConfig::new().baudrate(100.kHz().into());
    let mut i2c_driver = I2cDriver::new(peripherals.i2c0, sda, scl, &i2c_config)?;

    // Delay provider for ES8311 init
    let mut delay = Ets;

    // Address 0x18 is the standard default factory I2C address for ES8311
    let codec = Es8311::new(0x18);
    codec
        .init(
            &mut i2c_driver,
            &es8311::ClockConfig {
                mclk_inverted: false,
                sclk_inverted: false,
                mclk_from_mclk_pin: true,
                mclk_frequency: MCLK_FREQ,
                sample_frequency: SAMPLE_RATE,
            },
            es8311::Resolution::Bits16,
            es8311::Resolution::Bits16,
            &mut delay,
        )
        .map_err(|e| anyhow::anyhow!("ES8311 init error: {e:?}"))?;
    codec
        .volume_set(&mut i2c_driver, 70, None)
        .map_err(|e| anyhow::anyhow!("ES8311 volume_set error: {e:?}"))?;
    codec
        .mute(&mut i2c_driver, false)
        .map_err(|e| anyhow::anyhow!("ES8311 mute error: {e:?}"))?;

    // ==========================================
    // 3. INITIALIZE I2S PORT
    // ==========================================
    let i2s_config = config::StdConfig::new(
        config::Config::default(),
        config::StdClkConfig::from_sample_rate_hz(SAMPLE_RATE),
        config::StdSlotConfig::philips_slot_default(
            config::DataBitWidth::Bits16,
            config::SlotMode::Mono,
        ),
        config::StdGpioConfig::default(),
    );

    let mut i2s_tx = I2sDriver::<I2sTx>::new_std_tx(
        peripherals.i2s0,
        &i2s_config,
        bclk,
        dout,
        Some(mclk), // Pass the MCLK pin here
        ws,
    )?;

    // Turn on the external power amplifier circuit now that the clocks are stable
    pa_pin.set_low()?;
    println!("ES8311 Audio Subsystem Ready.");

    // ==========================================
    // 4. PCM STREAMING LOOP
    // ==========================================
    // Dummy 16-bit PCM Audio data array (Replace with raw audio binaries compiled in)
    let pcm_data: &[u8] = &[0x00; 1024];

    loop {
        // Feed the hardware ring-buffer continuously
        i2s_tx.write(pcm_data, BLOCK)?;
        FreeRtos::delay_ms(10);
    }
}
