use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::*;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::adc::oneshot::config::AdcChannelConfig;
use esp_idf_hal::adc::oneshot::{AdcChannelDriver, AdcDriver};
use esp_idf_hal::adc::attenuation::DB_12; // Tetap menggunakan DB_12 sesuai dengan target board kamu
use std::time::Instant;

#[derive(Copy, Clone, PartialEq)]
enum SystemState {
    Normal,
    DangerDigital,
    DangerAnalog,
}

fn main() -> anyhow::Result<()> {
    // Menghubungkan patch sistem agar ESP-IDF berjalan optimal
    esp_idf_svc::sys::link_patches();

    let peripherals = Peripherals::take()?;

    // 1. Konfigurasi Pin Digital (Switch & LED)
    let mut switch = PinDriver::input(peripherals.pins.gpio5, Pull::Up)?;
    
    // Konfigurasi LED Eksternal di GPIO 7
    let mut led = PinDriver::output(peripherals.pins.gpio7)?;

    // 2. Konfigurasi Pin Analog (Potensiometer)
    let adc1 = AdcDriver::new(peripherals.adc1)?;
    let channel_cfg = AdcChannelConfig {
        attenuation: DB_12,
        ..Default::default()
    };
    let mut potensio = AdcChannelDriver::new(&adc1, peripherals.pins.gpio6, &channel_cfg)?;

    println!("=== ASMC-Rust (Versi Oneshot-STD - GPIO 7) Berhasil Dijalankan ===");
    
    // Header CSV persis seperti di dokumen Bab 3.3
    println!("Waktu(ms),Status_Digital,Nilai_Analog,Latensi(us)");
    
    let mut waktu_ms: u32 = 0;

    loop {
        // Mulai hitung latensi asinkron
        let start = Instant::now(); 

        // Baca input sensor
        let is_switch_on = switch.is_high();
        let adc_value = adc1.read(&mut potensio)?; // Membaca nilai ADC via driver oneshot

        let mut current_state = SystemState::Normal;

        // Logika State Machine
        if is_switch_on {
            current_state = SystemState::DangerDigital;
        } else if adc_value > 2048 {
            current_state = SystemState::DangerAnalog;
        }

        // Aktuasi LED berdasarkan State
        match current_state {
            SystemState::Normal => led.set_low()?,
            SystemState::DangerDigital | SystemState::DangerAnalog => led.set_high()?,
        }

        // Hitung latensi dalam mikrodetik
        let latensi_us = start.elapsed().as_micros();
        waktu_ms += 100;

        // Konversi boolean ke angka (0/1) untuk CSV
        let status_digital = if is_switch_on { 1 } else { 0 };
        
        // Print hasil pembacaan dalam format CSV
        println!("{},{},{},{}", waktu_ms, status_digital, adc_value, latensi_us);

        // Delay 100ms agar data tidak membanjiri terminal
        FreeRtos::delay_ms(100);
    }
}
