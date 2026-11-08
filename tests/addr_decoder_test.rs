use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

#[verilog(src = "rtl/addr_decoder.v", name = "addr_decoder")]
struct AddrDecoder;

// Карта адресов (docs/ISA.md, раздел «Память»), адреса в словах
const RAM_BASE: u32 = 0x0000_0000;
const RAM_SIZE: u32 = 1024;
const VRAM_BASE: u32 = 0x0000_1000;
const VRAM_SIZE: u32 = 4096;
const MMIO_BASE: u32 = 0xFFFF_FC00;
const MMIO_SIZE: u32 = 1024;

// Разные значения на шинах чтения, чтобы видеть, откуда пришло слово
const RAM_RD: u32 = 0x1111_1111;
const VRAM_RD: u32 = 0x2222_2222;
const MMIO_RD: u32 = 0x3333_3333;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Region {
    Ram,
    Vram,
    Mmio,
    None,
}

// Ожидаемая область для адреса: сравнение диапазонов, без разбора битов
fn region(addr: u32) -> Region {
    let in_range = |base: u32, size: u32| addr >= base && addr - base < size;
    if in_range(RAM_BASE, RAM_SIZE) {
        Region::Ram
    } else if in_range(VRAM_BASE, VRAM_SIZE) {
        Region::Vram
    } else if in_range(MMIO_BASE, MMIO_SIZE) {
        Region::Mmio
    } else {
        Region::None
    }
}

fn expected_read(r: Region) -> u32 {
    match r {
        Region::Ram => RAM_RD,
        Region::Vram => VRAM_RD,
        Region::Mmio => MMIO_RD,
        Region::None => 0,
    }
}

macro_rules! new_decoder {
    ($runtime:ident, $d:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/addr_decoder.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $d = $runtime.create_model_simple::<AddrDecoder>()?;
        $d.ram_rd = RAM_RD;
        $d.vram_rd = VRAM_RD;
        $d.mmio_rd = MMIO_RD;
        $d.we = 0;
    };
}

// Выставляет адрес и признак записи и вычисляет выходы
macro_rules! access {
    ($d:expr, $addr:expr, $we:expr) => {{
        $d.addr = $addr;
        $d.we = $we;
        $d.eval();
    }};
}

// Какая область получила запись по выходам we (None, если ни одна)
macro_rules! written_region {
    ($d:expr) => {{
        let mut hit = Vec::new();
        if $d.ram_we != 0 {
            hit.push(Region::Ram);
        }
        if $d.vram_we != 0 {
            hit.push(Region::Vram);
        }
        if $d.mmio_we != 0 {
            hit.push(Region::Mmio);
        }
        hit
    }};
}

#[test]
#[snafu::report]
fn region_boundaries() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    let cases = [
        // (адрес, область)
        (RAM_BASE, Region::Ram),
        (RAM_BASE + RAM_SIZE - 1, Region::Ram),
        (RAM_BASE + RAM_SIZE, Region::None),
        (VRAM_BASE - 1, Region::None),
        (VRAM_BASE, Region::Vram),
        (VRAM_BASE + VRAM_SIZE - 1, Region::Vram),
        (VRAM_BASE + VRAM_SIZE, Region::None),
        (MMIO_BASE - 1, Region::None),
        (MMIO_BASE, Region::Mmio),
        (MMIO_BASE + (MMIO_SIZE - 1), Region::Mmio), // 0xFFFFFFFF, последний адрес
    ];
    for (addr, expected) in cases {
        assert_eq!(region(addr), expected, "модель: адрес {addr:#010x}");
        access!(d, addr, 0);
        assert_eq!(d.rd, expected_read(expected), "чтение, адрес {addr:#010x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn every_address_of_each_region_is_selected() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    for (base, size, expected) in [
        (RAM_BASE, RAM_SIZE, Region::Ram),
        (VRAM_BASE, VRAM_SIZE, Region::Vram),
        (MMIO_BASE, MMIO_SIZE, Region::Mmio),
    ] {
        for offset in 0..size {
            let addr = base + offset;
            access!(d, addr, 1);
            assert_eq!(d.rd, expected_read(expected), "чтение, адрес {addr:#010x}");
            assert_eq!(written_region!(d), vec![expected], "запись, адрес {addr:#010x}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn address_inside_region_is_the_low_bits() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    for offset in 0..RAM_SIZE {
        access!(d, RAM_BASE + offset, 0);
        assert_eq!(u32::from(d.ram_addr), offset, "RAM, смещение {offset}");
    }
    for offset in 0..VRAM_SIZE {
        access!(d, VRAM_BASE + offset, 0);
        assert_eq!(u32::from(d.vram_addr), offset, "видеопамять, смещение {offset}");
    }
    for offset in 0..MMIO_SIZE {
        access!(d, MMIO_BASE + offset, 0);
        assert_eq!(u32::from(d.mmio_addr), offset, "MMIO, смещение {offset}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn every_value_of_the_upper_22_bits() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    // область зависит только от старших 22 битов адреса: перебираем все
    for high in 0..(1u32 << 22) {
        let addr = (high << 10) | 0x155;
        let expected = region(addr);
        access!(d, addr, 1);
        assert_eq!(d.rd, expected_read(expected), "чтение, адрес {addr:#010x}");
        let hit = written_region!(d);
        let want: Vec<Region> = if expected == Region::None { vec![] } else { vec![expected] };
        assert_eq!(hit, want, "запись, адрес {addr:#010x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn single_bit_and_single_hole_addresses() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    for bit in 0..32 {
        for addr in [1u32 << bit, !(1u32 << bit)] {
            let expected = region(addr);
            access!(d, addr, 1);
            assert_eq!(d.rd, expected_read(expected), "чтение, адрес {addr:#010x}");
            let want: Vec<Region> = if expected == Region::None { vec![] } else { vec![expected] };
            assert_eq!(written_region!(d), want, "запись, адрес {addr:#010x}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn unmapped_addresses_read_zero_and_ignore_writes() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    for addr in [
        0x0000_0400,
        0x0000_0FFF,
        0x0000_2000,
        0x0001_0000,
        0x0001_FFFF,
        0x1000_0000,
        0x8000_0000,
        0xFFFF_0000,
        0xFFFF_FBFF,
    ] {
        assert_eq!(region(addr), Region::None, "модель: адрес {addr:#010x}");
        access!(d, addr, 1);
        assert_eq!(d.rd, 0, "чтение, адрес {addr:#010x}");
        assert_eq!(written_region!(d), vec![], "запись, адрес {addr:#010x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_signals_without_we() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    for addr in [RAM_BASE, 0x3FF, VRAM_BASE, 0x1FFF, MMIO_BASE, 0xFFFF_FFFF, 0x5000] {
        access!(d, addr, 0);
        assert_eq!(written_region!(d), vec![], "адрес {addr:#010x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn read_comes_only_from_the_selected_region() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    // меняем шины чтения невыбранных областей: результат не должен зависеть от них
    for (addr, expected) in [
        (0x010, Region::Ram),
        (0x1234, Region::Vram),
        (0xFFFF_FE00, Region::Mmio),
    ] {
        for (a, b, c) in [(1u32, 2, 3), (0xFFFF_FFFF, 0, 0xAAAA_AAAA), (0, 0, 0)] {
            d.ram_rd = a;
            d.vram_rd = b;
            d.mmio_rd = c;
            access!(d, addr, 0);
            let want = match expected {
                Region::Ram => a,
                Region::Vram => b,
                Region::Mmio => c,
                Region::None => 0,
            };
            assert_eq!(d.rd, want, "адрес {addr:#010x}, шины {a:#x} {b:#x} {c:#x}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn mmio_is_reachable_by_negative_offset_from_r0() -> Result<(), VerilatorError> {
    new_decoder!(runtime, d);
    // LOAD/STORE rd, imm(R0) с imm = -1024..-1: адрес = знаковое расширение imm
    for imm in -1024i32..0 {
        let addr = imm as u32;
        access!(d, addr, 1);
        assert_eq!(d.rd, MMIO_RD, "imm = {imm}");
        assert_eq!(written_region!(d), vec![Region::Mmio], "imm = {imm}");
        assert_eq!(u32::from(d.mmio_addr), (imm + 1024) as u32, "imm = {imm}");
    }
    // а imm = -1025 уже за пределами MMIO
    access!(d, (-1025i32) as u32, 1);
    assert_eq!(d.rd, 0);
    assert_eq!(written_region!(d), vec![]);
    Ok(())
}
