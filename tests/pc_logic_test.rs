use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

#[verilog(src = "rtl/pc_logic.v", name = "pc_logic")]
struct PcLogic;

const MASK: u16 = 0x3FF; // адреса по модулю 1024

// pc_src (docs/ISA.md, «Слово микрокоманды»)
const SRC_NEXT: u8 = 0; // PC + 1
const SRC_REL: u8 = 1; // PC + imm
const SRC_REG: u8 = 2; // rs1
const SRC_NONE: u8 = 3; // не используется

// pc_cond
const COND_ALWAYS: u8 = 0;
const COND_Z1: u8 = 1;
const COND_Z0: u8 = 2;
const COND_C1: u8 = 3;
const COND_C0: u8 = 4;

macro_rules! new_logic {
    ($runtime:ident, $l:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/pc_logic.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $l = $runtime.create_model_simple::<PcLogic>()?;
    };
}

// Подаёт входы и возвращает (next, we)
macro_rules! run {
    ($l:expr, pc=$pc:expr, imm=$imm:expr, rs1=$rs1:expr, z=$z:expr, c=$c:expr,
     we=$we:expr, src=$src:expr, cond=$cond:expr) => {{
        $l.pc = $pc;
        $l.imm = $imm;
        $l.rs1 = $rs1;
        $l.z = $z;
        $l.c = $c;
        $l.pc_we = $we;
        $l.pc_src = $src;
        $l.pc_cond = $cond;
        $l.eval();
        ($l.next, $l.we)
    }};
}

#[test]
#[snafu::report]
fn pc_plus_one_for_every_pc() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    for pc in 0..=MASK {
        let (next, _) = run!(l, pc = pc, imm = 0, rs1 = 0, z = 0, c = 0, we = 1, src = SRC_NEXT, cond = COND_ALWAYS);
        assert_eq!(next, (pc + 1) & MASK, "pc = {pc}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn pc_wraps_from_last_address_to_zero() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    let (next, _) = run!(l, pc = 1023, imm = 0, rs1 = 0, z = 0, c = 0, we = 1, src = SRC_NEXT, cond = COND_ALWAYS);
    assert_eq!(next, 0);
    Ok(())
}

#[test]
#[snafu::report]
fn relative_jump_for_every_pc_and_offset() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    for pc in 0..=MASK {
        for imm in 0..=MASK {
            let (next, _) = run!(l, pc = pc, imm = imm, rs1 = 0, z = 0, c = 0, we = 1, src = SRC_REL, cond = COND_ALWAYS);
            assert_eq!(next, (pc + imm) & MASK, "pc = {pc}, imm = {imm}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn relative_jump_examples() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    // (pc, imm как 10 бит, ожидаемый адрес); imm = 0x3FF это -1
    for (pc, imm, expected) in [
        (5u16, 3u16, 8u16),      // вперёд
        (5, 0x3FF, 4),           // назад на 1
        (0, 0x3FF, 1023),        // назад через ноль
        (1000, 100, 76),         // вперёд через конец памяти
        (7, 0, 7),               // на месте
        (10, 0x3F6, 0),          // -10 от 10 даёт 0
    ] {
        let (next, _) = run!(l, pc = pc, imm = imm, rs1 = 0, z = 0, c = 0, we = 1, src = SRC_REL, cond = COND_ALWAYS);
        assert_eq!(next, expected, "pc = {pc}, imm = {imm:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn register_jump_takes_rs1_for_every_value() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    for rs1 in 0..=MASK {
        // pc и imm заведомо отличаются от rs1
        let (next, _) = run!(l, pc = 0x155, imm = 0x2AA, rs1 = rs1, z = 0, c = 0, we = 1, src = SRC_REG, cond = COND_ALWAYS);
        assert_eq!(next, rs1, "rs1 = {rs1}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn condition_table() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    // (pc_cond, z, c, ожидается запись)
    let table = [
        (COND_ALWAYS, 0, 0, 1), (COND_ALWAYS, 0, 1, 1), (COND_ALWAYS, 1, 0, 1), (COND_ALWAYS, 1, 1, 1),
        (COND_Z1, 0, 0, 0), (COND_Z1, 0, 1, 0), (COND_Z1, 1, 0, 1), (COND_Z1, 1, 1, 1),
        (COND_Z0, 0, 0, 1), (COND_Z0, 0, 1, 1), (COND_Z0, 1, 0, 0), (COND_Z0, 1, 1, 0),
        (COND_C1, 0, 0, 0), (COND_C1, 0, 1, 1), (COND_C1, 1, 0, 0), (COND_C1, 1, 1, 1),
        (COND_C0, 0, 0, 1), (COND_C0, 0, 1, 0), (COND_C0, 1, 0, 1), (COND_C0, 1, 1, 0),
        // неиспользуемые условия никогда не разрешают запись
        (5, 0, 0, 0), (5, 1, 1, 0), (6, 0, 1, 0), (6, 1, 0, 0), (7, 0, 0, 0), (7, 1, 1, 0),
    ];
    for (cond, z, c, expected) in table {
        let (_, we) = run!(l, pc = 10, imm = 3, rs1 = 0, z = z, c = c, we = 1, src = SRC_REL, cond = cond);
        assert_eq!(we, expected, "pc_cond = {cond}, Z = {z}, C = {c}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn no_write_without_pc_we() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    for src in 0..4u8 {
        for cond in 0..8u8 {
            for z in 0..2u8 {
                for c in 0..2u8 {
                    let (_, we) = run!(l, pc = 10, imm = 3, rs1 = 7, z = z, c = c, we = 0, src = src, cond = cond);
                    assert_eq!(we, 0, "src = {src}, cond = {cond}, Z = {z}, C = {c}");
                }
            }
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn unused_pc_src_never_writes() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    for cond in 0..8u8 {
        let (_, we) = run!(l, pc = 10, imm = 3, rs1 = 7, z = 1, c = 1, we = 1, src = SRC_NONE, cond = cond);
        assert_eq!(we, 0, "pc_src = 3, pc_cond = {cond}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn next_address_does_not_depend_on_flags_or_condition() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    let (expected, _) = run!(l, pc = 100, imm = 20, rs1 = 300, z = 0, c = 0, we = 1, src = SRC_REL, cond = COND_ALWAYS);
    assert_eq!(expected, 120);
    for cond in 0..8u8 {
        for z in 0..2u8 {
            for c in 0..2u8 {
                let (next, _) = run!(l, pc = 100, imm = 20, rs1 = 300, z = z, c = c, we = 1, src = SRC_REL, cond = cond);
                assert_eq!(next, expected, "cond = {cond}, Z = {z}, C = {c}");
            }
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn commands_behave_as_in_isa() -> Result<(), VerilatorError> {
    new_logic!(runtime, l);
    // (команда, pc_src, pc_cond, z, c, ожидается запись, ожидаемый адрес) при pc = 50, imm = 5, rs1 = 200
    let cases = [
        ("выборка", SRC_NEXT, COND_ALWAYS, 0, 0, 1, 51),
        ("JUMP", SRC_REL, COND_ALWAYS, 0, 0, 1, 55),
        ("JAL", SRC_REL, COND_ALWAYS, 1, 1, 1, 55),
        ("JR", SRC_REG, COND_ALWAYS, 0, 0, 1, 200),
        ("BEQ, равны", SRC_REL, COND_Z1, 1, 0, 1, 55),
        ("BEQ, не равны", SRC_REL, COND_Z1, 0, 0, 0, 55),
        ("BNE, не равны", SRC_REL, COND_Z0, 0, 0, 1, 55),
        ("BNE, равны", SRC_REL, COND_Z0, 1, 0, 0, 55),
        ("BLTU, меньше", SRC_REL, COND_C1, 0, 1, 1, 55),
        ("BLTU, не меньше", SRC_REL, COND_C1, 0, 0, 0, 55),
        ("BGEU, не меньше", SRC_REL, COND_C0, 0, 0, 1, 55),
        ("BGEU, меньше", SRC_REL, COND_C0, 0, 1, 0, 55),
    ];
    for (name, src, cond, z, c, expected_we, expected_next) in cases {
        let (next, we) = run!(l, pc = 50, imm = 5, rs1 = 200, z = z, c = c, we = 1, src = src, cond = cond);
        assert_eq!(we, expected_we, "{name}: запись");
        assert_eq!(next, expected_next, "{name}: адрес");
    }
    Ok(())
}
