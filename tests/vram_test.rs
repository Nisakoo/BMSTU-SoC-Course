use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

#[verilog(src = "rtl/vram.v", name = "vram")]
struct Vram;

const SIZE: u16 = 4096;

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

// Запись слова через порт процессора на одном такте; после неё we = 0
macro_rules! write_word {
    ($m:expr, $addr:expr, $value:expr) => {{
        $m.addr = $addr;
        $m.wd = $value;
        $m.we = 1;
        tick!($m);
        $m.we = 0;
    }};
}

// Чтение через порт процессора
macro_rules! read_cpu {
    ($m:expr, $addr:expr) => {{
        $m.addr = $addr;
        $m.eval();
        $m.rd
    }};
}

// Чтение через порт дисплея
macro_rules! read_display {
    ($m:expr, $addr:expr) => {{
        $m.raddr = $addr;
        $m.eval();
        $m.rdata
    }};
}

macro_rules! new_vram {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/vram.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<Vram>()?;
        $m.we = 0;
        $m.addr = 0;
        $m.wd = 0;
        $m.raddr = 0;
    };
}

#[test]
#[snafu::report]
fn all_words_are_zero_at_start_on_both_ports() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    for addr in 0..SIZE {
        assert_eq!(read_cpu!(m, addr), 0, "порт процессора, адрес {addr:#05x}");
        assert_eq!(read_display!(m, addr), 0, "порт дисплея, адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_then_read_every_address_on_both_ports() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    for addr in 0..SIZE {
        write_word!(m, addr, pattern(addr));
    }
    for addr in 0..SIZE {
        assert_eq!(read_cpu!(m, addr), pattern(addr), "порт процессора, адрес {addr:#05x}");
        assert_eq!(read_display!(m, addr), pattern(addr), "порт дисплея, адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_changes_only_the_target_word() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    write_word!(m, 0x555, 0xDEAD_BEEF);
    for addr in 0..SIZE {
        let expected = if addr == 0x555 { 0xDEAD_BEEF } else { 0 };
        assert_eq!(read_display!(m, addr), expected, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn first_and_last_address_are_writable() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    write_word!(m, 0, 0x1111_1111);
    write_word!(m, SIZE - 1, 0x2222_2222);
    assert_eq!(read_cpu!(m, 0), 0x1111_1111, "первый адрес");
    assert_eq!(read_display!(m, SIZE - 1), 0x2222_2222, "последний адрес");
    Ok(())
}

#[test]
#[snafu::report]
fn each_address_bit_selects_its_own_word() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    for bit in 0..12 {
        let addr: u16 = 1 << bit;
        write_word!(m, addr, pattern(addr));
    }
    for bit in 0..12 {
        let addr: u16 = 1 << bit;
        assert_eq!(read_display!(m, addr), pattern(addr), "бит адреса {bit}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn all_32_bits_are_stored() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    for (i, value) in [0xFFFF_FFFF, 0xAAAA_AAAA, 0x5555_5555, 0x8000_0000, 0x0000_0001]
        .into_iter()
        .enumerate()
    {
        write_word!(m, 10, value);
        assert_eq!(read_cpu!(m, 10), value, "порт процессора, значение №{i}");
        assert_eq!(read_display!(m, 10), value, "порт дисплея, значение №{i}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn four_pixel_bytes_are_kept_independently() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    // слово из четырёх пикселей: байт 0 в битах [7:0], байт 3 в [31:24]
    write_word!(m, 0x021, 0x44_33_22_11);
    let word = read_display!(m, 0x021);
    assert_eq!(word & 0xFF, 0x11, "пиксель 0");
    assert_eq!((word >> 8) & 0xFF, 0x22, "пиксель 1");
    assert_eq!((word >> 16) & 0xFF, 0x33, "пиксель 2");
    assert_eq!((word >> 24) & 0xFF, 0x44, "пиксель 3");
    Ok(())
}

#[test]
#[snafu::report]
fn second_write_overwrites_first() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    write_word!(m, 7, 111);
    write_word!(m, 7, 222);
    assert_eq!(read_display!(m, 7), 222);
    write_word!(m, 7, 0);
    assert_eq!(read_display!(m, 7), 0, "можно записать ноль");
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_when_we_is_low() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    write_word!(m, 3, 0x1111_1111);
    m.addr = 3;
    m.wd = 0x2222_2222;
    m.we = 0;
    tick!(m);
    tick!(m);
    assert_eq!(read_display!(m, 3), 0x1111_1111, "we = 0: запись не должна пройти");
    Ok(())
}

#[test]
#[snafu::report]
fn both_read_ports_are_combinational() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    write_word!(m, 1, 10);
    write_word!(m, 2, 20);
    // clk не меняется: меняем только адреса и вызываем eval
    m.addr = 1;
    m.raddr = 2;
    m.eval();
    assert_eq!(m.rd, 10, "порт процессора");
    assert_eq!(m.rdata, 20, "порт дисплея");
    m.addr = 2;
    m.raddr = 1;
    m.eval();
    assert_eq!(m.rd, 20, "порт процессора после смены адреса");
    assert_eq!(m.rdata, 10, "порт дисплея после смены адреса");
    Ok(())
}

#[test]
#[snafu::report]
fn ports_have_independent_addresses() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    for addr in [0u16, 1, 0x155, 0x2AA, 0xFFF] {
        write_word!(m, addr, pattern(addr));
    }
    // адрес одного порта не влияет на другой
    for (a, b) in [(0u16, 0xFFFu16), (0x155, 0x2AA), (0x2AA, 0x155), (1, 1), (0xFFF, 0)] {
        m.addr = a;
        m.raddr = b;
        m.eval();
        assert_eq!(m.rd, pattern(a), "порт процессора, адрес {a:#05x}, порт дисплея {b:#05x}");
        assert_eq!(m.rdata, pattern(b), "порт дисплея, адрес {b:#05x}, порт процессора {a:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_happens_only_on_rising_edge() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    write_word!(m, 4, 100); // после записи clk = 1

    m.addr = 4;
    m.raddr = 4;
    m.wd = 200;
    m.we = 1;
    m.eval(); // clk уже 1, фронта не было
    assert_eq!(m.rd, 100, "порт процессора: we = 1 без фронта clk не пишет");
    assert_eq!(m.rdata, 100, "порт дисплея: то же");

    m.clk = 0;
    m.eval();
    assert_eq!(m.rd, 100, "спад clk не пишет");
    assert_eq!(m.rdata, 100);

    m.clk = 1;
    m.eval();
    assert_eq!(m.rd, 200, "фронт clk 0 -> 1 записывает");
    assert_eq!(m.rdata, 200, "порт дисплея видит новое значение");
    Ok(())
}

#[test]
#[snafu::report]
fn display_port_does_not_change_the_contents() -> Result<(), VerilatorError> {
    new_vram!(runtime, m);
    for addr in 0..SIZE {
        write_word!(m, addr, pattern(addr));
    }
    // двойной полный обход через порт дисплея и несколько тактов: содержимое то же
    for _ in 0..2 {
        for addr in 0..SIZE {
            m.raddr = addr;
            tick!(m);
        }
    }
    for addr in 0..SIZE {
        assert_eq!(read_cpu!(m, addr), pattern(addr), "адрес {addr:#05x}");
    }
    Ok(())
}
