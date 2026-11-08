use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Дисплей отдельно: проверка проводки между Rust и портом видеопамяти
#[verilog(src = "rtl/dev_display.v", name = "dev_display")]
struct DevDisplay;

// Видеопамять и дисплей вместе, как в soc_top: tests/rtl/display_vram.v
#[verilog(src = "tests/rtl/display_vram.v", name = "display_vram")]
struct DisplayVram;

const WORDS: u16 = 4096;
const WIDTH: usize = 128;
const HEIGHT: usize = 128;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

macro_rules! new_display {
    ($runtime:ident, $d:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_display.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $d = $runtime.create_model_simple::<DevDisplay>()?;
    };
}

macro_rules! new_pair {
    ($runtime:ident, $m:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &[
                "rtl/vram.v".as_ref(),
                "rtl/dev_display.v".as_ref(),
                "tests/rtl/display_vram.v".as_ref(),
            ],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $m = $runtime.create_model_simple::<DisplayVram>()?;
        $m.we = 0;
        $m.addr = 0;
        $m.wd = 0;
        $m.frame_addr = 0;
        $m.bus_addr = 0;
        $m.bus_we = 0;
        $m.bus_wd = 0;
    };
}

// Запись слова в видеопамять со стороны процессора
macro_rules! cpu_write {
    ($m:expr, $addr:expr, $value:expr) => {{
        $m.addr = $addr;
        $m.wd = $value;
        $m.we = 1;
        tick!($m);
        $m.we = 0;
    }};
}

// Чтение слова кадра со стороны Rust (через дисплей)
macro_rules! frame_word {
    ($m:expr, $addr:expr) => {{
        $m.frame_addr = $addr;
        $m.eval();
        $m.frame_data
    }};
}

// Цвет пикселя тестовой картинки: разный для соседних пикселей и строк
fn color(x: usize, y: usize) -> u8 {
    ((x * 7 + y * 13) & 0xFF) as u8
}

// Слово видеопамяти с номером w из четырёх пикселей: пиксель 0 в битах [7:0]
fn word_of_image(w: usize) -> u32 {
    let n = w * 4; // номер первого пикселя слова
    (0..4).fold(0u32, |acc, i| {
        let (x, y) = ((n + i) % WIDTH, (n + i) / WIDTH);
        acc | (u32::from(color(x, y)) << (8 * i))
    })
}

#[test]
#[snafu::report]
fn display_passes_the_address_to_vram_for_every_word() -> Result<(), VerilatorError> {
    new_display!(runtime, d);
    for addr in 0..WORDS {
        d.frame_addr = addr;
        d.eval();
        assert_eq!(d.vram_raddr, addr, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn display_passes_the_word_back_to_rust() -> Result<(), VerilatorError> {
    new_display!(runtime, d);
    for value in [0u32, 1, 0xFFFF_FFFF, 0xAAAA_AAAA, 0x5555_5555, 0x8000_0000, 0x44_33_22_11] {
        d.vram_rdata = value;
        d.eval();
        assert_eq!(d.frame_data, value, "слово {value:#010x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn display_bus_has_no_readable_registers() -> Result<(), VerilatorError> {
    new_display!(runtime, d);
    d.vram_rdata = 0xFFFF_FFFF;
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
fn bus_signals_do_not_affect_the_video_path() -> Result<(), VerilatorError> {
    new_display!(runtime, d);
    d.frame_addr = 0x155;
    d.vram_rdata = 0xCAFE_BABE;
    for (addr, we, wd) in [(0u16, 0u8, 0u32), (0x2AA, 1, 0xFFFF_FFFF), (0x3FF, 1, 0)] {
        d.addr = addr;
        d.we = we;
        d.wd = wd;
        d.eval();
        assert_eq!(d.vram_raddr, 0x155);
        assert_eq!(d.frame_data, 0xCAFE_BABE);
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
        assert_eq!(frame_word!(m, addr), 0xC0DE_0000 | u32::from(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn frame_is_zero_until_the_cpu_writes() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    for addr in 0..WORDS {
        assert_eq!(frame_word!(m, addr), 0, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn frame_is_reassembled_into_the_same_image() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    // процессор записывает картинку: пиксель (x, y) имеет номер y*128 + x,
    // лежит в слове y*32 + x/4, в байте x mod 4
    for w in 0..usize::from(WORDS) {
        cpu_write!(m, w as u16, word_of_image(w));
    }
    // Rust читает кадр через дисплей и достаёт пиксели по правилу из ISA.md
    let mut frame = vec![0u32; usize::from(WORDS)];
    for (w, slot) in frame.iter_mut().enumerate() {
        *slot = frame_word!(m, w as u16);
    }
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let word = frame[y * 32 + x / 4];
            let pixel = ((word >> (8 * (x % 4))) & 0xFF) as u8;
            assert_eq!(pixel, color(x, y), "пиксель ({x}, {y})");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn single_pixel_changes_only_its_byte() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    // пиксель (5, 3): слово 3*32 + 5/4 = 97, байт 5 mod 4 = 1 (биты [15:8])
    cpu_write!(m, 97, 0x0000_AB00);
    for addr in 0..WORDS {
        let expected = if addr == 97 { 0x0000_AB00 } else { 0 };
        assert_eq!(frame_word!(m, addr), expected, "адрес {addr:#05x}");
    }
    let pixel = (frame_word!(m, 97) >> 8) & 0xFF;
    assert_eq!(pixel, 0xAB);
    Ok(())
}

#[test]
#[snafu::report]
fn cpu_and_display_ports_work_at_the_same_time() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    cpu_write!(m, 10, 0x1010_1010);
    cpu_write!(m, 20, 0x2020_2020);
    // процессор читает слово 10, а дисплей в этот же момент слово 20
    m.addr = 10;
    m.frame_addr = 20;
    m.eval();
    assert_eq!(m.rd, 0x1010_1010, "процессор");
    assert_eq!(m.frame_data, 0x2020_2020, "дисплей");
    Ok(())
}

#[test]
#[snafu::report]
fn reading_the_frame_does_not_change_the_vram() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    for addr in 0..WORDS {
        cpu_write!(m, addr, 0xA000_0000 | u32::from(addr));
    }
    // несколько полных обходов кадра без тактов, потом процессор читает всё сам
    for _ in 0..3 {
        for addr in 0..WORDS {
            let _ = frame_word!(m, addr);
        }
    }
    for addr in 0..WORDS {
        m.addr = addr;
        m.eval();
        assert_eq!(m.rd, 0xA000_0000 | u32::from(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn display_sees_a_write_only_after_the_clock_edge() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    cpu_write!(m, 4, 100); // после записи clk = 1
    m.frame_addr = 4;
    m.addr = 4;
    m.wd = 200;
    m.we = 1;
    m.eval(); // фронта clk не было
    assert_eq!(m.frame_data, 100, "до фронта clk дисплей видит старое слово");
    m.clk = 0;
    m.eval();
    assert_eq!(m.frame_data, 100, "спад clk слово не меняет");
    m.clk = 1;
    m.eval();
    assert_eq!(m.frame_data, 200, "после фронта clk дисплей видит новое слово");
    Ok(())
}

#[test]
#[snafu::report]
fn display_bus_writes_do_not_reach_the_vram() -> Result<(), VerilatorError> {
    new_pair!(runtime, m);
    cpu_write!(m, 5, 0x5555_5555);
    // на порту процессора остаются другие адрес и данные: если бы запись по шине
    // дисплея доходила до видеопамяти, слово 6 изменилось бы
    m.addr = 6;
    m.wd = 0x7777_7777;
    // запись по шине устройства дисплея не должна менять видеопамять
    for addr in [0u16, 5, 0x30, 0x3FF] {
        m.bus_addr = addr;
        m.bus_we = 1;
        m.bus_wd = 0xFFFF_FFFF;
        tick!(m);
    }
    m.bus_we = 0;
    for addr in 0..WORDS {
        let expected = if addr == 5 { 0x5555_5555 } else { 0 };
        assert_eq!(frame_word!(m, addr), expected, "адрес {addr:#05x}");
    }
    assert_eq!(m.bus_rd, 0);
    Ok(())
}
