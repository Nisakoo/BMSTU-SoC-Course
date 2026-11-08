use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Таймер с параметрами по умолчанию: регистр по адресу 0x000, 50 тактов на микросекунду
#[verilog(src = "rtl/dev_timer.v", name = "dev_timer")]
struct DevTimer;

// Таймер с делителем 3 и регистром по адресу 0x020: tests/rtl/dev_timer_fast.v
#[verilog(src = "tests/rtl/dev_timer_fast.v", name = "dev_timer_fast")]
struct DevTimerFast;

const DIV_FAST: u32 = 3;
const BASE_FAST: u16 = 0x020;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

macro_rules! new_timer {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_timer.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<DevTimer>()?;
        $m.reset = 1;
        $m.we = 0;
        $m.wd = 0;
        $m.addr = 0;
        tick!($m);
        $m.reset = 0;
    };
}

macro_rules! new_fast {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_timer.v".as_ref(), "tests/rtl/dev_timer_fast.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<DevTimerFast>()?;
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
fn default_divider_is_50_clocks_per_microsecond() -> Result<(), VerilatorError> {
    new_timer!(runtime, t);
    assert_eq!(t.time_us, 0, "после сброса");
    for _ in 0..49 {
        tick!(t);
    }
    assert_eq!(t.time_us, 0, "после 49 тактов");
    tick!(t);
    assert_eq!(t.time_us, 1, "после 50 тактов");
    for _ in 0..49 {
        tick!(t);
    }
    assert_eq!(t.time_us, 1, "после 99 тактов");
    tick!(t);
    assert_eq!(t.time_us, 2, "после 100 тактов");
    Ok(())
}

#[test]
#[snafu::report]
fn time_is_zero_after_reset() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    assert_eq!(t.time_us, 0);
    Ok(())
}

#[test]
#[snafu::report]
fn increments_every_divider_clocks() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    for clock in 1..=300u32 {
        tick!(t);
        assert_eq!(t.time_us, clock / DIV_FAST, "такт {clock}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn every_step_is_exactly_plus_one() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    let mut previous = t.time_us;
    let mut increments = 0;
    for clock in 1..=3000u32 {
        tick!(t);
        let now = t.time_us;
        assert!(now == previous || now == previous + 1, "такт {clock}: {previous} -> {now}");
        if now != previous {
            increments += 1;
        }
        previous = now;
    }
    assert_eq!(increments, 3000 / DIV_FAST, "число увеличений");
    assert_eq!(t.time_us, 1000);
    Ok(())
}

#[test]
#[snafu::report]
fn counter_is_wider_than_16_bits() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    // время должно пройти значение 65535 -> 65536 -> 65537 без усечения
    for _ in 0..(65_537 * DIV_FAST) {
        tick!(t);
    }
    assert_eq!(t.time_us, 65_537);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_clears_time_and_divider() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    for _ in 0..(5 * DIV_FAST + 1) {
        tick!(t); // 5 микросекунд и один лишний такт делителя
    }
    assert_eq!(t.time_us, 5);
    t.reset = 1;
    tick!(t);
    t.reset = 0;
    assert_eq!(t.time_us, 0, "время после сброса");
    for _ in 0..(DIV_FAST - 1) {
        tick!(t);
    }
    assert_eq!(t.time_us, 0, "делитель тоже сброшен: увеличения раньше {DIV_FAST} тактов быть не должно");
    tick!(t);
    assert_eq!(t.time_us, 1);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_is_synchronous() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    for _ in 0..(4 * DIV_FAST) {
        tick!(t);
    }
    assert_eq!(t.time_us, 4);
    t.reset = 1;
    t.eval(); // фронта clk не было
    assert_eq!(t.time_us, 4, "reset без фронта clk не сбрасывает");
    tick!(t);
    assert_eq!(t.time_us, 0, "сброс по фронту clk");
    Ok(())
}

#[test]
#[snafu::report]
fn register_is_readable_only_at_its_address() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    for _ in 0..(7 * DIV_FAST) {
        tick!(t);
    }
    assert_eq!(t.time_us, 7);
    for addr in 0..1024u16 {
        t.addr = addr;
        t.eval();
        let expected = if addr == BASE_FAST { 7 } else { 0 };
        assert_eq!(t.rd, expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn default_register_address_is_zero() -> Result<(), VerilatorError> {
    new_timer!(runtime, t);
    for _ in 0..50 {
        tick!(t);
    }
    for addr in 0..1024u16 {
        t.addr = addr;
        t.eval();
        let expected = if addr == 0 { 1 } else { 0 };
        assert_eq!(t.rd, expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn bus_read_follows_the_counter() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    t.addr = BASE_FAST;
    for clock in 1..=60u32 {
        tick!(t);
        assert_eq!(t.rd, clock / DIV_FAST, "такт {clock}");
        assert_eq!(t.rd, t.time_us, "rd и time_us должны совпадать");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn writes_do_not_change_the_timer() -> Result<(), VerilatorError> {
    new_fast!(runtime, t);
    for clock in 1..=90u32 {
        // запись в регистр таймера и в другие адреса на каждом такте
        t.we = 1;
        t.wd = 0xFFFF_FFFF;
        t.addr = if clock % 2 == 0 { BASE_FAST } else { (clock % 1024) as u16 };
        tick!(t);
        assert_eq!(t.time_us, clock / DIV_FAST, "такт {clock}");
    }
    Ok(())
}
