use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Привод ворот с параметрами по умолчанию: регистр по адресу 0x040
#[verilog(src = "rtl/dev_gate.v", name = "dev_gate")]
struct DevGate;

const BASE: u16 = 0x040;

const OPEN: u32 = 0b01;
const CLOSE: u32 = 0b10;

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

macro_rules! new_gate {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_gate.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<DevGate>()?;
        $m.reset = 1;
        $m.we = 0;
        $m.wd = 0;
        $m.addr = 0;
        tick!($m);
        $m.reset = 0;
    };
}

// Выводы привода: (открыть, закрыть)
macro_rules! motor {
    ($m:expr) => {
        ($m.gate_open, $m.gate_close)
    };
}

#[test]
#[snafu::report]
fn motor_is_stopped_after_reset() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    assert_eq!(motor!(m), (0, 0));
    assert_eq!(read_at!(m, BASE), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn open_drives_only_the_open_output() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN);
    assert_eq!(motor!(m), (1, 0));
    assert_eq!(read_at!(m, BASE), OPEN);
    Ok(())
}

#[test]
#[snafu::report]
fn close_drives_only_the_close_output() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, CLOSE);
    assert_eq!(motor!(m), (0, 1));
    assert_eq!(read_at!(m, BASE), CLOSE);
    Ok(())
}

#[test]
#[snafu::report]
fn zero_stops_the_motor() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN);
    write_at!(m, BASE, 0);
    assert_eq!(motor!(m), (0, 0));
    assert_eq!(read_at!(m, BASE), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn both_bits_stop_the_motor_but_are_read_back() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN | CLOSE);
    assert_eq!(motor!(m), (0, 0), "защита: оба направления сразу невозможны");
    assert_eq!(read_at!(m, BASE), OPEN | CLOSE, "записанное значение читается как есть");
    Ok(())
}

#[test]
#[snafu::report]
fn direction_changes_directly() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN);
    assert_eq!(motor!(m), (1, 0));
    write_at!(m, BASE, CLOSE);
    assert_eq!(motor!(m), (0, 1));
    write_at!(m, BASE, OPEN);
    assert_eq!(motor!(m), (1, 0));
    Ok(())
}

#[test]
#[snafu::report]
fn only_bits_0_and_1_are_used() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, 0xFFFF_FFFC);
    assert_eq!(motor!(m), (0, 0), "биты выше 1 игнорируются");
    assert_eq!(read_at!(m, BASE), 0);
    write_at!(m, BASE, 0xFFFF_FFFD);
    assert_eq!(motor!(m), (1, 0));
    assert_eq!(read_at!(m, BASE), OPEN);
    write_at!(m, BASE, 0xFFFF_FFFE);
    assert_eq!(motor!(m), (0, 1));
    assert_eq!(read_at!(m, BASE), CLOSE);
    Ok(())
}

#[test]
#[snafu::report]
fn write_only_at_its_address() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    for addr in 0..1024u16 {
        write_at!(m, BASE, 0);
        write_at!(m, addr, OPEN);
        let expected = if addr == BASE { (1, 0) } else { (0, 0) };
        assert_eq!(motor!(m), expected, "запись по адресу {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn register_is_readable_only_at_its_address() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, CLOSE);
    for addr in 0..1024u16 {
        let expected = if addr == BASE { CLOSE } else { 0 };
        assert_eq!(read_at!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_without_we() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN);
    m.addr = BASE;
    m.wd = CLOSE;
    m.we = 0;
    tick!(m);
    tick!(m);
    assert_eq!(motor!(m), (1, 0), "we = 0: состояние не меняется");
    Ok(())
}

#[test]
#[snafu::report]
fn write_happens_only_on_rising_edge() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN); // после записи clk = 1
    m.addr = BASE;
    m.wd = CLOSE;
    m.we = 1;
    m.eval(); // фронта clk не было
    assert_eq!(motor!(m), (1, 0), "we = 1 без фронта clk не пишет");
    m.clk = 0;
    m.eval();
    assert_eq!(motor!(m), (1, 0), "спад clk не пишет");
    m.clk = 1;
    m.eval();
    assert_eq!(motor!(m), (0, 1), "фронт clk 0 -> 1 записывает");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_stops_the_motor() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, OPEN);
    m.reset = 1;
    tick!(m);
    m.reset = 0;
    assert_eq!(motor!(m), (0, 0));
    assert_eq!(read_at!(m, BASE), 0);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_is_synchronous() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    write_at!(m, BASE, CLOSE);
    m.reset = 1;
    m.eval(); // фронта clk не было
    assert_eq!(motor!(m), (0, 1), "reset без фронта clk не сбрасывает");
    tick!(m);
    assert_eq!(motor!(m), (0, 0), "сброс по фронту clk");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_beats_write() -> Result<(), VerilatorError> {
    new_gate!(runtime, m);
    m.reset = 1;
    m.addr = BASE;
    m.wd = OPEN;
    m.we = 1;
    tick!(m);
    assert_eq!(motor!(m), (0, 0), "при reset и we одновременно побеждает reset");
    Ok(())
}
