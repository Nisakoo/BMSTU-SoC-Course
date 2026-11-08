use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Обёртка с фиксированной шириной 32 лежит в tests/rtl/
#[verilog(src = "tests/rtl/reg_we32.v", name = "reg_we32")]
struct RegWe32;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

// Запись значения на одном такте; после неё we = 0
macro_rules! write_value {
    ($m:expr, $value:expr) => {{
        $m.d = $value;
        $m.we = 1;
        tick!($m);
        $m.we = 0;
    }};
}

// Создаёт runtime и модель регистра и делает начальный сброс
macro_rules! new_reg {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/reg_we.v".as_ref(), "tests/rtl/reg_we32.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<RegWe32>()?;
        $m.reset = 1;
        $m.we = 0;
        $m.d = 0;
        tick!($m);
        $m.reset = 0;
    };
}

#[test]
#[snafu::report]
fn value_is_zero_after_reset() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    assert_eq!(m.q, 0);
    Ok(())
}

#[test]
#[snafu::report]
fn write_stores_value() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 0x1234_5678);
    assert_eq!(m.q, 0x1234_5678);
    Ok(())
}

#[test]
#[snafu::report]
fn all_32_bits_are_stored() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    for (i, value) in [
        0xFFFF_FFFF,
        0xAAAA_AAAA,
        0x5555_5555,
        0x8000_0000,
        0x0000_0001,
        0x0000_0000,
    ]
    .into_iter()
    .enumerate()
    {
        write_value!(m, value);
        assert_eq!(m.q, value, "значение №{i}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn each_bit_is_stored_independently() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    for bit in 0..32 {
        let value: u32 = 1 << bit;
        write_value!(m, value);
        assert_eq!(m.q, value, "бит {bit}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn second_write_overwrites_first() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 111);
    write_value!(m, 222);
    assert_eq!(m.q, 222);
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_when_we_is_low() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 0x1111_1111);
    m.d = 0x2222_2222;
    m.we = 0;
    tick!(m);
    assert_eq!(m.q, 0x1111_1111, "we = 0: значение не должно измениться");
    Ok(())
}

#[test]
#[snafu::report]
fn value_is_held_over_many_clocks() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 0xCAFE_BABE);
    for i in 0..100u32 {
        m.d = i.wrapping_mul(0x0101_0101); // на входе меняются данные, но we = 0
        tick!(m);
        assert_eq!(m.q, 0xCAFE_BABE, "такт {i}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_after_idle_clocks_works() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 1);
    m.d = 2;
    tick!(m); // we = 0
    m.d = 3;
    m.we = 1;
    tick!(m);
    assert_eq!(m.q, 3, "после тактов без записи запись снова работает");
    Ok(())
}

#[test]
#[snafu::report]
fn write_happens_only_on_rising_edge() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 100); // после записи clk = 1

    m.d = 200;
    m.we = 1;
    m.eval(); // clk уже 1, фронта не было
    assert_eq!(m.q, 100, "we = 1 без фронта clk не пишет");

    m.clk = 0;
    m.eval();
    assert_eq!(m.q, 100, "спад clk не пишет");

    m.clk = 1;
    m.eval();
    assert_eq!(m.q, 200, "фронт clk 0 -> 1 записывает");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_clears_stored_value() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 0xDEAD_BEEF);
    m.reset = 1;
    tick!(m);
    assert_eq!(m.q, 0);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_is_synchronous() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 0x6666_6666);
    m.reset = 1;
    m.eval(); // фронта clk не было
    assert_eq!(m.q, 0x6666_6666, "reset без фронта clk не сбрасывает");
    tick!(m);
    assert_eq!(m.q, 0, "сброс по фронту clk");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_beats_write() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 5);
    m.reset = 1;
    m.we = 1;
    m.d = 0x8888_8888;
    tick!(m);
    assert_eq!(m.q, 0, "при reset и we одновременно побеждает reset");
    Ok(())
}

#[test]
#[snafu::report]
fn write_works_again_after_reset() -> Result<(), VerilatorError> {
    new_reg!(runtime, m);
    write_value!(m, 0x1111_1111);
    m.reset = 1;
    tick!(m);
    m.reset = 0;
    write_value!(m, 0x2222_2222);
    assert_eq!(m.q, 0x2222_2222);
    Ok(())
}
