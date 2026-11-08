use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Устройство просмотра отдельно: проверка проводки между Rust и портом RAM
#[verilog(src = "rtl/dev_ram_view.v", name = "dev_ram_view")]
struct DevRamView;

// RAM и устройство просмотра вместе, как в soc_top: tests/rtl/ram_view_pair.v
#[verilog(src = "tests/rtl/ram_view_pair.v", name = "ram_view_pair")]
struct RamViewPair;

const WORDS: u16 = 1024;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

macro_rules! new_view {
    ($runtime:ident, $d:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_ram_view.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $d = $runtime.create_model_simple::<DevRamView>()?;
    };
}

macro_rules! new_pair {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &[
                "rtl/ram.v".as_ref(),
                "rtl/dev_ram_view.v".as_ref(),
                "tests/rtl/ram_view_pair.v".as_ref(),
            ],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<RamViewPair>()?;
        $m.we = 0;
        $m.addr = 0;
        $m.wd = 0;
        $m.view_addr = 0;
        $m.bus_addr = 0;
        $m.bus_we = 0;
        $m.bus_wd = 0;
    };
}

// Запись слова в RAM со стороны процессора
macro_rules! cpu_write {
    ($m:expr, $addr:expr, $value:expr) => {{
        $m.addr = $addr;
        $m.wd = $value;
        $m.we = 1;
        tick!($m);
        $m.we = 0;
    }};
}

// Чтение слова RAM со стороны Rust (через устройство)
macro_rules! view_word {
    ($m:expr, $addr:expr) => {{
        $m.view_addr = $addr;
        $m.eval();
        $m.view_data
    }};
}

#[test]
#[snafu::report]
fn device_passes_the_address_to_ram_for_every_word() -> Result<(), VerilatorError> {
    new_view!(runtime, d);
    for addr in 0..WORDS {
        d.view_addr = addr;
        d.eval();
        assert_eq!(d.ram_raddr, addr, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn device_passes_the_word_back_to_rust() -> Result<(), VerilatorError> {
    new_view!(runtime, d);
    for value in [0u32, 1, 0xFFFF_FFFF, 0xAAAA_AAAA, 0x5555_5555, 0x8000_0000] {
        d.ram_rdata = value;
        d.eval();
        assert_eq!(d.view_data, value, "слово {value:#010x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn device_bus_has_no_readable_registers() -> Result<(), VerilatorError> {
    new_view!(runtime, d);
    d.ram_rdata = 0xFFFF_FFFF;
    d.wd = 0xFFFF_FFFF;
    for we in 0..2u8 {
        d.we = we;
        for addr in 0..1024u16 {
            d.addr = addr;
            d.eval();
            assert_eq!(d.rd, 0, "адрес {addr:#05x}, we = {we}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn bus_signals_do_not_affect_the_view_path() -> Result<(), VerilatorError> {
    new_view!(runtime, d);
    d.view_addr = 0x155;
    d.ram_rdata = 0xCAFE_BABE;
    for (addr, we, wd) in [(0u16, 0u8, 0u32), (0x2AA, 1, 0xFFFF_FFFF), (0x3FF, 1, 0)] {
        d.addr = addr;
        d.we = we;
        d.wd = wd;
        d.eval();
        assert_eq!(d.ram_raddr, 0x155);
        assert_eq!(d.view_data, 0xCAFE_BABE);
    }
    Ok(())
}

#[test]
#[snafu::report]
fn rust_reads_what_the_cpu_wrote_at_every_address() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    for addr in 0..WORDS {
        cpu_write!(m, addr, 0xC0DE_0000 | u32::from(addr));
    }
    for addr in 0..WORDS {
        assert_eq!(view_word!(m, addr), 0xC0DE_0000 | u32::from(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn ram_is_zero_until_the_cpu_writes() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    for addr in 0..WORDS {
        assert_eq!(view_word!(m, addr), 0, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn cpu_and_view_ports_work_at_the_same_time() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    cpu_write!(m, 10, 0x1010_1010);
    cpu_write!(m, 20, 0x2020_2020);
    // процессор читает слово 10, а Rust в этот же момент слово 20
    m.addr = 10;
    m.view_addr = 20;
    m.eval();
    assert_eq!(m.data, 0x1010_1010, "процессор");
    assert_eq!(m.view_data, 0x2020_2020, "Rust");
    Ok(())
}

#[test]
#[snafu::report]
fn reading_through_the_device_does_not_change_the_ram() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    for addr in 0..WORDS {
        cpu_write!(m, addr, 0xA000_0000 | u32::from(addr));
    }
    for _ in 0..3 {
        for addr in 0..WORDS {
            let _ = view_word!(m, addr);
        }
    }
    for addr in 0..WORDS {
        m.addr = addr;
        m.eval();
        assert_eq!(m.data, 0xA000_0000 | u32::from(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn rust_sees_a_write_only_after_the_clock_edge() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    cpu_write!(m, 4, 100); // после записи clk = 1
    m.view_addr = 4;
    m.addr = 4;
    m.wd = 200;
    m.we = 1;
    m.eval(); // фронта clk не было
    assert_eq!(m.view_data, 100, "до фронта clk Rust видит старое слово");
    m.clk = 0;
    m.eval();
    assert_eq!(m.view_data, 100, "спад clk слово не меняет");
    m.clk = 1;
    m.eval();
    assert_eq!(m.view_data, 200, "после фронта clk Rust видит новое слово");
    Ok(())
}

#[test]
#[snafu::report]
fn device_bus_writes_do_not_reach_the_ram() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    cpu_write!(m, 5, 0x5555_5555);
    // на порту процессора остаются другие адрес и данные: если бы запись по шине
    // устройства доходила до RAM, слово 6 изменилось бы
    m.addr = 6;
    m.wd = 0x7777_7777;
    for addr in [0u16, 5, 0x30, 0x3FF] {
        m.bus_addr = addr;
        m.bus_we = 1;
        m.bus_wd = 0xFFFF_FFFF;
        tick!(m);
    }
    m.bus_we = 0;
    for addr in 0..WORDS {
        let expected = if addr == 5 { 0x5555_5555 } else { 0 };
        assert_eq!(view_word!(m, addr), expected, "адрес {addr:#05x}");
    }
    assert_eq!(m.bus_rd, 0);
    Ok(())
}
