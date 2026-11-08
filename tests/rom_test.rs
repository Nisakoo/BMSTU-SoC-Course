use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Обёртки лежат в tests/rtl/. Файл с программой: tests/data/rom_test.hex
#[verilog(src = "tests/rtl/rom_program.v", name = "rom_program")]
struct RomProgram;

#[verilog(src = "tests/rtl/rom_empty.v", name = "rom_empty")]
struct RomEmpty;

const ROM_SIZE: u16 = 1024;

// Слова, записанные в tests/data/rom_test.hex: (адрес, значение).
// Остальные ячейки должны читаться как 0.
// Таблица должна совпадать с hex-файлом.
const PROGRAM: &[(u16, u32)] = &[
    (0x000, 0xDEAD_BEEF),
    // адреса с одним установленным битом: значение = 0xA000_0000 | адрес
    (0x001, 0xA000_0001),
    (0x002, 0xA000_0002),
    (0x004, 0xA000_0004),
    (0x008, 0xA000_0008),
    (0x010, 0xA000_0010),
    (0x020, 0xA000_0020),
    (0x040, 0xA000_0040),
    (0x080, 0xA000_0080),
    (0x100, 0xA000_0100),
    (0x200, 0xA000_0200),
    // чередующиеся биты адреса
    (0x155, 0x1111_1155),
    (0x2AA, 0x2222_22AA),
    // последний адрес
    (0x3FF, 0xFFFF_FFFF),
];

fn expected(addr: u16) -> u32 {
    PROGRAM
        .iter()
        .find(|(a, _)| *a == addr)
        .map_or(0, |(_, value)| *value)
}

// Создаёт runtime и модель ROM с программой
macro_rules! new_rom {
    ($runtime:ident, $rom:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/rom.v".as_ref(), "tests/rtl/rom_program.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $rom = $runtime.create_model_simple::<RomProgram>()?;
    };
}

fn read(rom: &mut RomProgram<'_>, addr: u16) -> u32 {
    rom.addr = addr;
    rom.eval();
    rom.data
}

#[test]
#[snafu::report]
fn stored_words_are_read_back() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    for (addr, value) in PROGRAM {
        assert_eq!(read(&mut rom, *addr), *value, "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn unlisted_addresses_are_zero() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    for addr in 0..ROM_SIZE {
        assert_eq!(read(&mut rom, addr), expected(addr), "адрес {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn first_and_last_address_are_readable() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    assert_eq!(read(&mut rom, 0), 0xDEAD_BEEF, "первый адрес");
    assert_eq!(read(&mut rom, ROM_SIZE - 1), 0xFFFF_FFFF, "последний адрес");
    Ok(())
}

#[test]
#[snafu::report]
fn each_address_bit_selects_its_own_word() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    for bit in 0..10 {
        let addr: u16 = 1 << bit;
        assert_eq!(
            read(&mut rom, addr),
            0xA000_0000 | u32::from(addr),
            "бит адреса {bit}"
        );
    }
    Ok(())
}

#[test]
#[snafu::report]
fn alternating_address_bits_are_not_swapped() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    assert_eq!(read(&mut rom, 0x155), 0x1111_1155);
    assert_eq!(read(&mut rom, 0x2AA), 0x2222_22AA);
    Ok(())
}

#[test]
#[snafu::report]
fn read_is_combinational() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    // у ROM нет тактового сигнала: данные должны меняться сразу за адресом
    rom.addr = 0x001;
    rom.eval();
    assert_eq!(rom.data, 0xA000_0001);
    rom.addr = 0x3FF;
    rom.eval();
    assert_eq!(rom.data, 0xFFFF_FFFF, "после смены адреса");
    rom.addr = 0x001;
    rom.eval();
    assert_eq!(rom.data, 0xA000_0001, "возврат к первому адресу");
    Ok(())
}

#[test]
#[snafu::report]
fn reading_does_not_change_contents() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    for _ in 0..2 {
        for (addr, value) in PROGRAM {
            assert_eq!(read(&mut rom, *addr), *value, "адрес {addr:#05x}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn rom_without_file_is_all_zeros() -> Result<(), VerilatorError> {
    let runtime = VerilatorRuntime::new(
        "build".as_ref(),
        &["rtl/rom.v".as_ref(), "tests/rtl/rom_empty.v".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )?;
    let mut rom = runtime.create_model_simple::<RomEmpty>()?;
    for addr in 0..ROM_SIZE {
        rom.addr = addr;
        rom.eval();
        assert_eq!(rom.data, 0, "адрес {addr:#05x}");
    }
    Ok(())
}
