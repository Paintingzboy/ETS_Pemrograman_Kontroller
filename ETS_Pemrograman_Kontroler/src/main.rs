#![no_std]
#![no_main]

use esp_idf_sys as _; // Inisialisasi driver level rendah ESP-IDF

use esp_idf_hal::adc::{AdcChannelDriver, AdcDriver, Attenuation};
use esp_idf_hal::delay::{Ets, FreeRtos};
use esp_idf_hal::gpio::{PinDriver, Pull};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::hal::prelude::*;
use core::fmt::Write;

// Macro untuk mendefinisikan entry point aplikasi embedded Rust
#[no_mangle]
fn main() {
    // 1. Ambil kendali penuh atas semua periferal internal hardware ESP32-S3
    let peripherals = Peripherals::take().unwrap();
    
    // 2. Inisialisasi Serial UART0 (CDC USB virtual internal)
    let mut serial = esp_idf_svc::hal::uart::UartDriver::new(
        peripherals.uart0,
        peripherals.pins.gpio43, // TX pin internal
        peripherals.pins.gpio44, // RX pin internal
        &esp_idf_svc::hal::uart::config::Config::default().baudrate(115200.into()),
    ).unwrap();

    // Beri jeda 1.5 detik agar port koneksi virtual stabil (Sama seperti delay(1500) di C++)
    FreeRtos::delay_ms(1500);

    // 3. Konfigurasi Pin Digital Input (GPIO 5 - Switch)
    let mut sensor_digital = PinDriver::input(peripherals.pins.gpio5).unwrap();
    // Gunakan konfigurasi Floating (tanpa pullup/pulldown internal) mengikuti diagram C++
    sensor_digital.set_pull(Pull::Floating).unwrap();

    // 4. Konfigurasi Pin Digital Output (GPIO 4 - LED)
    let mut led_pin = PinDriver::output(peripherals.pins.gpio4).unwrap();
    led_pin.set_low().unwrap(); // Pastikan LED mati saat boot

    // 5. Konfigurasi Pin Analog Input (GPIO 6 - Potensiometer)
    let adc_driver = AdcDriver::new(peripherals.adc1, &esp_idf_hal::adc::config::Config::new()).unwrap();
    // Mapping pin GPIO 6 ke channel ADC internal dengan atenuasi tegangan penuh (11 dB)
    let mut sensor_analog = AdcChannelDriver::<_, _>::new(
        &adc_driver,
        peripherals.pins.gpio6,
        &esp_idf_hal::adc::config::ChannelConfig::new().attenuation(Attenuation::DB_11),
    ).unwrap();

    // Cetak Header Tabel CSV ke Serial Monitor
    writeln!(serial, "Waktu(ms),Status_Digital,Nilai_Analog,Latensi(us)").unwrap();

    // Inisialisasi sistem pewaktu FreeRTOS untuk menghitung waktu operasi awal
    let start_system_tick = esp_idf_sys::esp_timer_get_time();

    // --- INFINITE LOOP (Fungsi Void Loop Asinkron) ---
    loop {
        // A. Catat waktu awal eksekusi mikrodetik (micros() di C++)
        let start_time = esp_idf_sys::esp_timer_get_time();

        // B. Pembacaan data sensor secara simultan (Non-blocking)
        let val_digital = if sensor_digital.is_high() { 1 } else { 0 };
        let val_analog: u16 = adc_driver.read(&mut sensor_analog).unwrap();

        // C. Logika evaluasi status keselamatan sistem (Threshold > 2500)
        let is_active = (val_digital == 1) || (val_analog > 2500);

        // D. Aktuasi instan pada komponen LED
        if is_active {
            led_pin.set_high().unwrap();
        } else {
            led_pin.set_low().unwrap();
        }

        // E. Hitung durasi latensi pemrosesan algoritma (t_end - t_start)
        let end_time = esp_idf_sys::esp_timer_get_time();
        let latensi = end_time - start_time;

        // F. Hitung total waktu berjalan sistem dalam milidetik (millis() di C++)
        let current_millis = (esp_idf_sys::esp_timer_get_time() - start_system_tick) / 1000;

        // G. Transmisikan data terformat CSV melalui Serial Monitor
        writeln!(
            serial,
            "{},{},{},{}",
            current_millis, val_digital, val_analog, latensi
        ).unwrap();

        // Jeda sampling rate sistem sebesar 100 milidetik (10 Hz)
        FreeRtos::delay_ms(100);
    }
}

// Handler khusus jika terjadi panic-error pada memory-safety Rust
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        Ets::delay_us(1000);
    }
}