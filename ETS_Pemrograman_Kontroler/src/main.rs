use anyhow::Result;
use esp_idf_hal::adc::attenuation::DB_11;
use esp_idf_hal::adc::oneshot::config::AdcChannelConfig;
use esp_idf_hal::adc::oneshot::{AdcChannelDriver, AdcDriver};
use esp_idf_hal::gpio::{PinDriver, Pull};
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::sys::{esp_timer_get_time, link_patches};

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

#[derive(Copy, Clone, PartialEq)]
enum SystemState {
    Normal,
    FireDetected,
    SmokeDetected,
    FireAndSmoke,
}

// Threshold MQ-7 — sesuaikan setelah warm-up 2-5 menit
const MQ7_THRESHOLD: u16 = 1500;

fn main() -> Result<()> {
    link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    // =========================================================
    // 1. Flame Sensor — GPIO 4, Digital Input (DO only)
    //    Active LOW: DO = LOW saat api terdeteksi
    //    Pull::Up supaya pin stabil HIGH saat idle
    // =========================================================
    let flame_do = PinDriver::input(peripherals.pins.gpio4, Pull::Up)?;

    // =========================================================
    // 2. MQ-7 — GPIO 5, ADC1 Analog Input
    //    DB_11 → Vref ~3.3V, cocok untuk output MQ-7
    //    ⚠️ VCC MQ-7 harus 5V/VIN, bukan 3.3V!
    // =========================================================
    let adc1 = AdcDriver::new(peripherals.adc1)?;
    let adc_config = AdcChannelConfig {
        attenuation: DB_11,
        ..Default::default()
    };
    let mut mq7_ao = AdcChannelDriver::new(&adc1, peripherals.pins.gpio5, &adc_config)?;

    // =========================================================
    // 3. Aktuator — LED GPIO 7, Buzzer GPIO 15
    // =========================================================
    let mut led    = PinDriver::output(peripherals.pins.gpio7)?;
    let mut buzzer = PinDriver::output(peripherals.pins.gpio15)?;
    led.set_low()?;
    buzzer.set_low()?;

    // =========================================================
    // 4. OLED SSD1306 — SDA: GPIO 17, SCL: GPIO 18
    // =========================================================
    let i2c_config = I2cConfig::new().baudrate(400_000.into());
    let i2c_driver = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio17, // SDA
        peripherals.pins.gpio18, // SCL
        &i2c_config,
    )?;
    let interface  = I2CDisplayInterface::new(i2c_driver);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();
    display.clear(BinaryColor::Off).unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    let mut waktu_ms: u64 = 0;

    println!("Waktu(ms),Flame_DO,MQ7_Raw,Status,Latensi(us)");

    loop {
        // START latensi
        let start_us = unsafe { esp_timer_get_time() };

        // A. Baca Flame Sensor (Digital)
        //    is_low() = true berarti DO aktif = api terdeteksi
        let ada_api = flame_do.is_low();

        // B. Baca MQ-7 (Analog)
        let mq7_raw = adc1.read(&mut mq7_ao).unwrap_or(0);
        let ada_co  = mq7_raw > MQ7_THRESHOLD;

        // C. Evaluasi state
        let current_state = match (ada_api, ada_co) {
            (true,  true)  => SystemState::FireAndSmoke,
            (true,  false) => SystemState::FireDetected,
            (false, true)  => SystemState::SmokeDetected,
            (false, false) => SystemState::Normal,
        };

        let status_str = match current_state {
            SystemState::Normal        => "NORMAL",
            SystemState::FireDetected  => "ALARM: API!",
            SystemState::SmokeDetected => "ALARM: CO/ASAP!",
            SystemState::FireAndSmoke  => "ALARM: API+CO!",
        };

        // D. Aktuasi LED + Buzzer
        match current_state {
            SystemState::Normal => {
                led.set_low()?;
                buzzer.set_low()?;
            }
            _ => {
                led.set_high()?;
                buzzer.set_high()?;
            }
        }

        // END latensi
        let end_us     = unsafe { esp_timer_get_time() };
        let latensi_us = (end_us - start_us) as u64;

        waktu_ms += 100;

        // Output CSV serial monitor
        println!(
            "{},{},{},{},{}",
            waktu_ms,
            ada_api as u8,
            mq7_raw,
            status_str,
            latensi_us
        );

        // ======================================================
        // REFRESH OLED
        // Baris 0 (y=0)  : Flame sensor status
        // Baris 1 (y=12) : MQ-7 raw value
        // Baris 2 (y=24) : Status sistem
        // Baris 3 (y=36) : Latensi
        // Baris 4 (y=48) : Waktu sistem
        // ======================================================
        display.clear(BinaryColor::Off).unwrap();

        // Baris 0: Flame
        let flame_label = if ada_api { "Flame: DETECTED!" } else { "Flame: Aman" };
        Text::with_baseline(flame_label, Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display).unwrap();

        // Baris 1: MQ-7
        let mut buf1: heapless::String<32> = heapless::String::new();
        core::fmt::write(&mut buf1, format_args!("MQ7 CO: {}", mq7_raw)).ok();
        Text::with_baseline(buf1.as_str(), Point::new(0, 12), text_style, Baseline::Top)
            .draw(&mut display).unwrap();

        // Baris 2: Status
        Text::with_baseline(status_str, Point::new(0, 24), text_style, Baseline::Top)
            .draw(&mut display).unwrap();

        // Baris 3: Latensi
        let mut buf3: heapless::String<32> = heapless::String::new();
        core::fmt::write(&mut buf3, format_args!("Lat: {} us", latensi_us)).ok();
        Text::with_baseline(buf3.as_str(), Point::new(0, 36), text_style, Baseline::Top)
            .draw(&mut display).unwrap();

        // Baris 4: Waktu
        let mut buf4: heapless::String<32> = heapless::String::new();
        core::fmt::write(&mut buf4, format_args!("t: {} ms", waktu_ms)).ok();
        Text::with_baseline(buf4.as_str(), Point::new(0, 48), text_style, Baseline::Top)
            .draw(&mut display).unwrap();

        display.flush().unwrap();

        FreeRtos::delay_ms(100);
    }
}
