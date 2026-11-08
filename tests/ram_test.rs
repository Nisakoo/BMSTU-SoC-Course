use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// RAM без файла: подключается напрямую
#[verilog(src = "rtl/ram.v", name = "ram")]
struct Ram;

// RAM с данными из tests/data/ram_test.hex: через обёртку в tests/rtl/
#[verilog(src = "tests/rtl/ram_program.v", name = "ram_program")]
struct RamProgram;

const RAM_SIZE: u16 = 1024;

// Слова, записанные в tests/data/ram_test.hex: (адрес, значение).
// Остальные ячейки должны читаться как 0.
// Таблица должна совпадать с hex-файлом.
const PRELOADED: &[(u16, u32)] = &[
    (0x000, 1),
    (0x001, 1),
    (0x002, 2),
    (0x003, 2),
    (0x004, 2),
    (0x005, 3),
    (0x006, 4),
    (0x007, 4),
    (0x3FF, 0xCAFE_BABE),
];

fn preloaded(addr: u16) -> u32 {
    PRELOADED
        .iter()
        .find(|(a, _)| *a == addr)
        .map_or(0, |(_, value)| *value)
}

// Уникальное значение для адреса: по нему видно, если ячейки перепутаны
fn pattern(addr: u16) -> u32 {
    0xC0DE_0000 | u32::from(addr)
}

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

// Запись слова на одном такте; после неё we = 0
macro_rules! write_word {
    ($m:expr, $addr:expr, $value:expr) => {{
        $m.addr = $addr;
        $m.wd = $value;
        $m.we = 1;
        tick!($m);
        $m.we = 0;
    }};
}

// Чтение слова по адресу
macro_rules! read_word {
    ($m:expr, $addr:expr) => {{
        $m.addr = $addr;
        $m.eval();
        $m.data
    }};
}

macro_rules! new_ram {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/ram.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<Ram>()?;
    };
}

macro_rules! new_ram_program {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/ram.v".as_ref(), "tests/rtl/ram_program.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<RamProgram>()?;
    };
}

// ---------- RAM без файла ----------

#[test]
#[snafu::report]
fn all_words_are_zero_without_file() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    for addr in 0..RAM_SIZE {
        assert_eq!(read_word!(m, addr), 0, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_then_read_every_address() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    for addr in 0..RAM_SIZE {
        write_word!(m, addr, pattern(addr));
    }
    // все значения на месте и не перепутаны между ячейками
    for addr in 0..RAM_SIZE {
        assert_eq!(read_word!(m, addr), pattern(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_changes_only_the_target_word() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    write_word!(m, 0x155, 0xDEAD_BEEF);
    for addr in 0..RAM_SIZE {
        let expected = if addr == 0x155 { 0xDEAD_BEEF } else { 0 };
        assert_eq!(read_word!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn first_and_last_address_are_writable() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    write_word!(m, 0, 0x1111_1111);
    write_word!(m, RAM_SIZE - 1, 0x2222_2222);
    assert_eq!(read_word!(m, 0), 0x1111_1111, "первый адрес");
    assert_eq!(read_word!(m, RAM_SIZE - 1), 0x2222_2222, "последний адрес");
    Ok(())
}

#[test]
#[snafu::report]
fn each_address_bit_selects_its_own_word() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    // адреса с одним установленным битом; запись в один не должна менять другие
    for bit in 0..10 {
        let addr: u16 = 1 << bit;
        write_word!(m, addr, pattern(addr));
    }
    for bit in 0..10 {
        let addr: u16 = 1 << bit;
        assert_eq!(read_word!(m, addr), pattern(addr), "бит адреса {bit}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn all_32_bits_are_stored() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    for (i, value) in [0xFFFF_FFFF, 0xAAAA_AAAA, 0x5555_5555, 0x8000_0000, 0x0000_0001]
        .into_iter()
        .enumerate()
    {
        write_word!(m, 10, value);
        assert_eq!(read_word!(m, 10), value, "значение №{i}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn second_write_overwrites_first() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    write_word!(m, 7, 111);
    write_word!(m, 7, 222);
    assert_eq!(read_word!(m, 7), 222);
    write_word!(m, 7, 0);
    assert_eq!(read_word!(m, 7), 0, "можно записать ноль");
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_when_we_is_low() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    write_word!(m, 3, 0x1111_1111);
    m.addr = 3;
    m.wd = 0x2222_2222;
    m.we = 0;
    tick!(m);
    tick!(m);
    assert_eq!(read_word!(m, 3), 0x1111_1111, "we = 0: запись не должна пройти");
    Ok(())
}

#[test]
#[snafu::report]
fn read_is_combinational() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    write_word!(m, 1, 10);
    write_word!(m, 2, 20);
    // clk не меняется: меняем только адрес и вызываем eval
    m.addr = 1;
    m.eval();
    assert_eq!(m.data, 10);
    m.addr = 2;
    m.eval();
    assert_eq!(m.data, 20, "значение сменилось без такта");
    Ok(())
}

#[test]
#[snafu::report]
fn write_happens_only_on_rising_edge() -> Result<(), VerilatorError> {
    new_ram!(runtime, m);
    write_word!(m, 4, 100); // после записи clk = 1

    m.addr = 4;
    m.wd = 200;
    m.we = 1;
    m.eval(); // clk уже 1, фронта не было
    assert_eq!(m.data, 100, "we = 1 без фронта clk не пишет");

    m.clk = 0;
    m.eval();
    assert_eq!(m.data, 100, "спад clk не пишет");

    m.clk = 1;
    m.eval();
    assert_eq!(m.data, 200, "фронт clk 0 -> 1 записывает");
    Ok(())
}

// ---------- RAM с данными из файла ----------

#[test]
#[snafu::report]
fn preloaded_words_are_read_back() -> Result<(), VerilatorError> {
    new_ram_program!(runtime, m);
    for (addr, value) in PRELOADED {
        assert_eq!(read_word!(m, *addr), *value, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn unlisted_addresses_are_zero_after_load() -> Result<(), VerilatorError> {
    new_ram_program!(runtime, m);
    for addr in 0..RAM_SIZE {
        assert_eq!(read_word!(m, addr), preloaded(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn clock_without_we_keeps_preloaded_data() -> Result<(), VerilatorError> {
    new_ram_program!(runtime, m);
    m.we = 0;
    m.wd = 0xFFFF_FFFF;
    for addr in 0..RAM_SIZE {
        m.addr = addr;
        tick!(m);
    }
    for addr in 0..RAM_SIZE {
        assert_eq!(read_word!(m, addr), preloaded(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_over_preloaded_word_changes_only_it() -> Result<(), VerilatorError> {
    new_ram_program!(runtime, m);
    write_word!(m, 0x002, 0x1234_5678); // было 2
    for addr in 0..RAM_SIZE {
        let expected = if addr == 0x002 { 0x1234_5678 } else { preloaded(addr) };
        assert_eq!(read_word!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}
