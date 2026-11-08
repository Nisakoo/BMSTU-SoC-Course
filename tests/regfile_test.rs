use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

#[verilog(src = "rtl/regfile.v", name = "regfile")]
struct RegFile;

// Один такт: clk 0 -> 1
fn tick(r: &mut RegFile<'_>) {
    r.clk = 0;
    r.eval();
    r.clk = 1;
    r.eval();
}

// Запись значения в регистр на одном такте; после неё we = 0
fn write(r: &mut RegFile<'_>, reg: u8, value: u32) {
    r.we = 1;
    r.wa = reg;
    r.wd = value;
    tick(r);
    r.we = 0;
}

// Чтение через порт 1 и порт 2
fn read1(r: &mut RegFile<'_>, reg: u8) -> u32 {
    r.ra1 = reg;
    r.eval();
    r.rd1
}

fn read2(r: &mut RegFile<'_>, reg: u8) -> u32 {
    r.ra2 = reg;
    r.eval();
    r.rd2
}

// Уникальное значение для регистра: по нему видно, если регистры перепутаны
fn pattern(reg: u8) -> u32 {
    0xA000_0000 | (u32::from(reg) * 0x0001_1111)
}

// Создаёт runtime и модель регистрового файла и делает начальный сброс
macro_rules! new_regfile {
    ($runtime:ident, $r:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/regfile.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $r = $runtime.create_model_simple::<RegFile>()?;
        $r.reset = 1;
        $r.we = 0;
        $r.wa = 0;
        $r.wd = 0;
        tick(&mut $r);
        $r.reset = 0;
    };
}

#[test]
#[snafu::report]
fn all_registers_are_zero_after_reset() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    for reg in 0..16 {
        assert_eq!(read1(&mut r, reg), 0, "порт 1, регистр {reg}");
        assert_eq!(read2(&mut r, reg), 0, "порт 2, регистр {reg}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_then_read_every_register() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    for reg in 1..16 {
        write(&mut r, reg, pattern(reg));
    }
    // все значения на месте и не перепутаны между регистрами
    for reg in 1..16 {
        assert_eq!(read1(&mut r, reg), pattern(reg), "порт 1, регистр {reg}");
        assert_eq!(read2(&mut r, reg), pattern(reg), "порт 2, регистр {reg}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn write_changes_only_the_target_register() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 5, 0xDEAD_BEEF);
    for reg in 0..16 {
        let expected = if reg == 5 { 0xDEAD_BEEF } else { 0 };
        assert_eq!(read1(&mut r, reg), expected, "регистр {reg}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn all_32_bits_are_stored() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    for (i, value) in [0xFFFF_FFFF, 0xAAAA_AAAA, 0x5555_5555, 0x8000_0000, 0x0000_0001]
        .into_iter()
        .enumerate()
    {
        write(&mut r, 3, value);
        assert_eq!(read1(&mut r, 3), value, "значение №{i}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn second_write_overwrites_first() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 7, 111);
    write(&mut r, 7, 222);
    assert_eq!(read1(&mut r, 7), 222);
    Ok(())
}

#[test]
#[snafu::report]
fn r0_always_reads_zero() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 0, 0xFFFF_FFFF);
    assert_eq!(read1(&mut r, 0), 0, "порт 1 после записи в R0");
    assert_eq!(read2(&mut r, 0), 0, "порт 2 после записи в R0");
    // запись в R0 не задевает другие регистры
    for reg in 1..16 {
        assert_eq!(read1(&mut r, reg), 0, "регистр {reg}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_when_we_is_low() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 3, 0x1111_1111);
    r.we = 0;
    r.wa = 3;
    r.wd = 0x2222_2222;
    tick(&mut r);
    assert_eq!(read1(&mut r, 3), 0x1111_1111, "we = 0: запись не должна пройти");
    Ok(())
}

#[test]
#[snafu::report]
fn two_read_ports_are_independent() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 1, 0x1111_1111);
    write(&mut r, 2, 0x2222_2222);

    r.ra1 = 1;
    r.ra2 = 2;
    r.eval();
    assert_eq!(r.rd1, 0x1111_1111, "порт 1 читает r1");
    assert_eq!(r.rd2, 0x2222_2222, "порт 2 читает r2");

    r.ra1 = 2;
    r.ra2 = 1;
    r.eval();
    assert_eq!(r.rd1, 0x2222_2222, "порты поменялись местами: порт 1 читает r2");
    assert_eq!(r.rd2, 0x1111_1111, "порты поменялись местами: порт 2 читает r1");

    r.ra1 = 2;
    r.ra2 = 2;
    r.eval();
    assert_eq!(r.rd1, 0x2222_2222, "один регистр на двух портах: порт 1");
    assert_eq!(r.rd2, 0x2222_2222, "один регистр на двух портах: порт 2");
    Ok(())
}

#[test]
#[snafu::report]
fn read_is_combinational() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 1, 10);
    write(&mut r, 2, 20);
    // clk не меняется: меняем только адрес и вызываем eval
    r.ra1 = 1;
    r.eval();
    assert_eq!(r.rd1, 10);
    r.ra1 = 2;
    r.eval();
    assert_eq!(r.rd1, 20, "значение сменилось без такта");
    Ok(())
}

#[test]
#[snafu::report]
fn write_happens_only_on_clock_edge() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 4, 100);

    r.we = 1;
    r.wa = 4;
    r.wd = 200;
    r.ra1 = 4;
    r.eval(); // фронта clk не было
    assert_eq!(r.rd1, 100, "до фронта clk видно старое значение");

    tick(&mut r);
    assert_eq!(r.rd1, 200, "после фронта clk видно новое значение");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_clears_all_registers() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    for reg in 1..16 {
        write(&mut r, reg, pattern(reg));
    }
    r.reset = 1;
    tick(&mut r);
    r.reset = 0;
    for reg in 0..16 {
        assert_eq!(read1(&mut r, reg), 0, "регистр {reg} после сброса");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn reset_is_synchronous() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    write(&mut r, 6, 0x6666_6666);
    r.reset = 1;
    r.eval(); // фронта clk не было
    assert_eq!(read1(&mut r, 6), 0x6666_6666, "reset без фронта clk не сбрасывает");
    tick(&mut r);
    assert_eq!(read1(&mut r, 6), 0, "сброс по фронту clk");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_beats_write() -> Result<(), VerilatorError> {
    new_regfile!(runtime, r);
    r.reset = 1;
    r.we = 1;
    r.wa = 8;
    r.wd = 0x8888_8888;
    tick(&mut r);
    assert_eq!(read1(&mut r, 8), 0, "при reset и we одновременно побеждает reset");
    Ok(())
}
