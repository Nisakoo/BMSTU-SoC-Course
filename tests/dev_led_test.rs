use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Один светодиод по адресу 0x030 и восемь по адресу 0x031: tests/rtl/
#[verilog(src = "tests/rtl/dev_led1.v", name = "dev_led1")]
struct Led1;

#[verilog(src = "tests/rtl/dev_led8.v", name = "dev_led8")]
struct Led8;

const BASE1: u16 = 0x030;
const BASE8: u16 = 0x031;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

// Запись слова по адресу на одном такте; после неё we = 0
macro_rules! write_at {
    ($m:expr, $addr:expr, $value:expr) => {{
        $m.addr = $addr;
        $m.wd = $value;
        $m.we = 1;
        tick!($m);
        $m.we = 0;
    }};
}

macro_rules! read_at {
    ($m:expr, $addr:expr) => {{
        $m.addr = $addr;
        $m.eval();
        $m.rd
    }};
}

macro_rules! new_led {
    ($runtime:ident, $m:ident, $ty:ty, $wrapper:expr) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_led.v".as_ref(), $wrapper.as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<$ty>()?;
        $m.reset = 1;
        $m.we = 0;
        $m.wd = 0;
        $m.addr = 0;
        tick!($m);
        $m.reset = 0;
    };
}

#[test]
#[snafu::report]
fn leds_are_off_after_reset() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    assert_eq!(m.led, 0);
    assert_eq!(read_at!(m, BASE8), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn every_value_of_eight_leds() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    for value in 0..=255u32 {
        write_at!(m, BASE8, value);
        assert_eq!(u32::from(m.led), value, "вывод, значение {value:#04x}");
        assert_eq!(read_at!(m, BASE8), value, "чтение, значение {value:#04x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn single_led_turns_on_and_off() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led1, "tests/rtl/dev_led1.v");
    write_at!(m, BASE1, 1);
    assert_eq!(m.led, 1);
    assert_eq!(read_at!(m, BASE1), 1);
    write_at!(m, BASE1, 0);
    assert_eq!(m.led, 0);
    assert_eq!(read_at!(m, BASE1), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn only_the_low_bits_are_used() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    write_at!(m, BASE8, 0xFFFF_FF5A);
    assert_eq!(m.led, 0x5A, "старшие биты записи игнорируются");
    assert_eq!(read_at!(m, BASE8), 0x5A, "и при чтении их нет");
    new_led!(runtime1, one, Led1, "tests/rtl/dev_led1.v");
    write_at!(one, BASE1, 0xFFFF_FFFE);
    assert_eq!(one.led, 0, "у одного светодиода значим только бит 0");
    Ok(())
}

#[test]
#[snafu::report]
fn write_only_at_its_address() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    for addr in 0..1024u16 {
        // чистое состояние перед каждым адресом
        write_at!(m, BASE8, 0);
        write_at!(m, addr, 0xFF);
        let expected = if addr == BASE8 { 0xFF } else { 0 };
        assert_eq!(u32::from(m.led), expected, "запись по адресу {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn register_is_readable_only_at_its_address() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    write_at!(m, BASE8, 0xA5);
    for addr in 0..1024u16 {
        let expected = if addr == BASE8 { 0xA5 } else { 0 };
        assert_eq!(read_at!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_without_we() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    write_at!(m, BASE8, 0x11);
    m.addr = BASE8;
    m.wd = 0xFF;
    m.we = 0;
    tick!(m);
    tick!(m);
    assert_eq!(m.led, 0x11, "we = 0: значение не меняется");
    Ok(())
}

#[test]
#[snafu::report]
fn write_happens_only_on_rising_edge() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    write_at!(m, BASE8, 0x01); // после записи clk = 1
    m.addr = BASE8;
    m.wd = 0x02;
    m.we = 1;
    m.eval(); // фронта clk не было
    assert_eq!(m.led, 0x01, "we = 1 без фронта clk не пишет");
    m.clk = 0;
    m.eval();
    assert_eq!(m.led, 0x01, "спад clk не пишет");
    m.clk = 1;
    m.eval();
    assert_eq!(m.led, 0x02, "фронт clk 0 -> 1 записывает");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_turns_leds_off() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    write_at!(m, BASE8, 0xFF);
    m.reset = 1;
    tick!(m);
    m.reset = 0;
    assert_eq!(m.led, 0);
    assert_eq!(read_at!(m, BASE8), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_is_synchronous() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    write_at!(m, BASE8, 0xFF);
    m.reset = 1;
    m.eval(); // фронта clk не было
    assert_eq!(m.led, 0xFF, "reset без фронта clk не сбрасывает");
    tick!(m);
    assert_eq!(m.led, 0, "сброс по фронту clk");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_beats_write() -> Result<(), VerilatorError> {
    new_led!(runtime, m, Led8, "tests/rtl/dev_led8.v");
    m.reset = 1;
    m.addr = BASE8;
    m.wd = 0xFF;
    m.we = 1;
    tick!(m);
    assert_eq!(m.led, 0, "при reset и we одновременно побеждает reset");
    Ok(())
}
