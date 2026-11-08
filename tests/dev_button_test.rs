use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Один вход по адресу 0x020 и четыре входа по адресу 0x025: tests/rtl/
#[verilog(src = "tests/rtl/dev_button1.v", name = "dev_button1")]
struct Button1;

#[verilog(src = "tests/rtl/dev_button4.v", name = "dev_button4")]
struct Button4;

const BASE1: u16 = 0x020;
const BASE4: u16 = 0x025;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

// Читает регистр по заданному адресу
macro_rules! read_at {
    ($m:expr, $addr:expr) => {{
        $m.addr = $addr;
        $m.eval();
        $m.rd
    }};
}

macro_rules! new_button {
    ($runtime:ident, $m:ident, $ty:ty, $wrapper:expr) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_button.v".as_ref(), $wrapper.as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<$ty>()?;
        $m.reset = 1;
        $m.we = 0;
        $m.wd = 0;
        $m.addr = 0;
        $m.btn = 0;
        tick!($m);
        $m.reset = 0;
    };
}

#[test]
#[snafu::report]
fn state_is_zero_after_reset() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    assert_eq!(read_at!(m, BASE1), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn press_is_visible_after_two_clocks() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    m.btn = 1;
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 0, "после первого такта (первый триггер синхронизатора)");
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1, "после второго такта");
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1, "кнопка удерживается");
    Ok(())
}

#[test]
#[snafu::report]
fn release_is_visible_after_two_clocks() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    m.btn = 1;
    tick!(m);
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1);
    m.btn = 0;
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1, "после первого такта состояние ещё прежнее");
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 0, "после второго такта");
    Ok(())
}

#[test]
#[snafu::report]
fn one_clock_pulse_is_seen_for_exactly_one_clock() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    m.btn = 1;
    tick!(m);
    m.btn = 0;
    assert_eq!(read_at!(m, BASE1), 0);
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1, "импульс дошёл до регистра");
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 0, "и ушёл");
    Ok(())
}

#[test]
#[snafu::report]
fn every_combination_of_four_inputs() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button4, "tests/rtl/dev_button4.v");
    for pattern in 0..16u8 {
        m.btn = pattern;
        tick!(m);
        tick!(m);
        assert_eq!(read_at!(m, BASE4), u32::from(pattern), "комбинация {pattern:#06b}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn inputs_are_independent_bits() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button4, "tests/rtl/dev_button4.v");
    for bit in 0..4 {
        m.btn = 1 << bit;
        tick!(m);
        tick!(m);
        assert_eq!(read_at!(m, BASE4), 1 << bit, "вход {bit}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn register_is_readable_only_at_its_address() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button4, "tests/rtl/dev_button4.v");
    m.btn = 0b1010;
    tick!(m);
    tick!(m);
    for addr in 0..1024u16 {
        let expected = if addr == BASE4 { 0b1010 } else { 0 };
        assert_eq!(read_at!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn single_input_register_address() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    m.btn = 1;
    tick!(m);
    tick!(m);
    for addr in 0..1024u16 {
        let expected = if addr == BASE1 { 1 } else { 0 };
        assert_eq!(read_at!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn writes_do_not_change_the_state() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button4, "tests/rtl/dev_button4.v");
    m.btn = 0b0101;
    tick!(m);
    tick!(m);
    // запись по адресу регистра идёт последней, чтобы следующий такт не затёр её следы
    for addr in [0, 0x3FF, BASE4] {
        m.addr = addr;
        m.we = 1;
        m.wd = 0xFFFF_FFFF;
        tick!(m);
        assert_eq!(read_at!(m, BASE4), 0b0101, "после записи по адресу {addr:#05x}");
    }
    m.we = 0;
    assert_eq!(read_at!(m, BASE4), 0b0101);
    // и после снятия кнопок состояние тоже определяется только входами
    m.btn = 0;
    tick!(m);
    tick!(m);
    assert_eq!(read_at!(m, BASE4), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn upper_bits_are_always_zero() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button4, "tests/rtl/dev_button4.v");
    for pattern in 0..16u8 {
        m.btn = pattern;
        tick!(m);
        tick!(m);
        assert_eq!(read_at!(m, BASE4) >> 4, 0, "комбинация {pattern:#06b}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn reset_clears_the_state_and_inputs_reappear() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    m.btn = 1;
    tick!(m);
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1);
    m.reset = 1;
    tick!(m);
    m.reset = 0;
    assert_eq!(read_at!(m, BASE1), 0, "сброс очистил состояние");
    // кнопка всё ещё нажата: через два такта состояние вернётся
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 0, "после первого такта: первый триггер тоже был сброшен");
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 1, "кнопка по-прежнему нажата");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_is_synchronous() -> Result<(), VerilatorError> {
    new_button!(runtime, m, Button1, "tests/rtl/dev_button1.v");
    m.btn = 1;
    tick!(m);
    tick!(m);
    m.reset = 1;
    m.eval(); // фронта clk не было
    assert_eq!(read_at!(m, BASE1), 1, "reset без фронта clk не сбрасывает");
    tick!(m);
    assert_eq!(read_at!(m, BASE1), 0, "сброс по фронту clk");
    Ok(())
}
