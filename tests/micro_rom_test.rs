use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

#[verilog(src = "rtl/micro_rom.v", name = "micro_rom")]
struct MicroRom;

// ---------- коды операций (docs/ISA.md, раздел «Команды») ----------
const HALT_OP: u8 = 0x00;
const ADD: u8 = 0x01;
const SUB: u8 = 0x02;
const AND: u8 = 0x03;
const OR: u8 = 0x04;
const XOR: u8 = 0x05;
const NOT: u8 = 0x06;
const SHL: u8 = 0x07;
const SHR: u8 = 0x08;
const SAR: u8 = 0x09;
const CMP: u8 = 0x0A;
const ADDI: u8 = 0x11;
const SUBI: u8 = 0x12;
const ANDI: u8 = 0x13;
const ORI: u8 = 0x14;
const XORI: u8 = 0x15;
const SHLI: u8 = 0x17;
const SHRI: u8 = 0x18;
const SARI: u8 = 0x19;
const CMPI: u8 = 0x1A;
const LOAD: u8 = 0x20;
const STORE: u8 = 0x21;
const JUMP: u8 = 0x30;
const BEQ: u8 = 0x31;
const BNE: u8 = 0x32;
const BLTU: u8 = 0x33;
const BGEU: u8 = 0x34;
const JAL: u8 = 0x35;
const JR: u8 = 0x36;
const HRT: u8 = 0x38; // HIGH_RES_TICK
const FETCH: u8 = 0x3F;

// ---------- слово микрокоманды (docs/ISA.md, «Слово микрокоманды») ----------
const fn bits(value: u32, lsb: u32) -> u32 {
    value << lsb
}

const LAST: u32 = bits(1, 21);
const HALT: u32 = bits(1, 20);
const MEM_WE: u32 = bits(1, 19);
const ADDR_WE: u32 = bits(1, 18);
const WB_CAPT: u32 = bits(0, 16);
const WB_PC: u32 = bits(1, 16);
const WB_MEM: u32 = bits(2, 16);
const WB_ALU: u32 = bits(3, 16);
const FLAGS_NOCAP: u32 = bits(1, 15);
const FLAGS_WE: u32 = bits(1, 14);
const ALU_ADD: u32 = bits(0, 10);
const ALU_SUB: u32 = bits(1, 10);
const ALU_AND: u32 = bits(2, 10);
const ALU_OR: u32 = bits(3, 10);
const ALU_XOR: u32 = bits(4, 10);
const ALU_NOT: u32 = bits(5, 10);
const ALU_SHL: u32 = bits(6, 10);
const ALU_SHR: u32 = bits(7, 10);
const ALU_SAR: u32 = bits(8, 10);
const B_IMM: u32 = bits(1, 9);
const REG_WE: u32 = bits(1, 8);
const RS2_RD: u32 = bits(1, 7);
const IR_WE: u32 = bits(1, 6);
const COND_ALWAYS: u32 = bits(0, 3);
const COND_Z1: u32 = bits(1, 3);
const COND_Z0: u32 = bits(2, 3);
const COND_C1: u32 = bits(3, 3);
const COND_C0: u32 = bits(4, 3);
const PC_NEXT: u32 = bits(0, 1);
const PC_REL: u32 = bits(1, 1);
const PC_REG: u32 = bits(2, 1);
const PC_WE: u32 = bits(1, 0);

// слово для кода и шагов, которых нет в таблице
const DEFAULT: u32 = HALT | LAST;

// результат АЛУ в rd, флаги обновляются, конец команды
const ALU_WRITE: u32 = WB_ALU | REG_WE | FLAGS_WE | LAST;
// первый шаг LOAD и STORE
const ADDR_CALC: u32 = ALU_ADD | B_IMM | ADDR_WE;
// переход на PC + imm
const JUMP_REL: u32 = PC_WE | PC_REL | LAST;

struct Row {
    name: &'static str,
    opcode: u8,
    step: u8,
    word: u32,
}

const fn row(name: &'static str, opcode: u8, step: u8, word: u32) -> Row {
    Row {
        name,
        opcode,
        step,
        word,
    }
}

// Все определённые шаги (docs/ISA.md, «Микропрограммы»)
const ROWS: &[Row] = &[
    row("FETCH", FETCH, 0, IR_WE | PC_WE | PC_NEXT | COND_ALWAYS | LAST),
    row("HALT", HALT_OP, 0, HALT | LAST),
    // АЛУ с регистром
    row("ADD", ADD, 0, ALU_ADD | ALU_WRITE),
    row("SUB", SUB, 0, ALU_SUB | ALU_WRITE),
    row("AND", AND, 0, ALU_AND | ALU_WRITE),
    row("OR", OR, 0, ALU_OR | ALU_WRITE),
    row("XOR", XOR, 0, ALU_XOR | ALU_WRITE),
    row("NOT", NOT, 0, ALU_NOT | ALU_WRITE),
    row("SHL", SHL, 0, ALU_SHL | ALU_WRITE),
    row("SHR", SHR, 0, ALU_SHR | ALU_WRITE),
    row("SAR", SAR, 0, ALU_SAR | ALU_WRITE),
    row("CMP", CMP, 0, ALU_SUB | FLAGS_WE | LAST),
    // АЛУ с константой
    row("ADDI", ADDI, 0, ALU_ADD | B_IMM | ALU_WRITE),
    row("SUBI", SUBI, 0, ALU_SUB | B_IMM | ALU_WRITE),
    row("ANDI", ANDI, 0, ALU_AND | B_IMM | ALU_WRITE),
    row("ORI", ORI, 0, ALU_OR | B_IMM | ALU_WRITE),
    row("XORI", XORI, 0, ALU_XOR | B_IMM | ALU_WRITE),
    row("SHLI", SHLI, 0, ALU_SHL | B_IMM | ALU_WRITE),
    row("SHRI", SHRI, 0, ALU_SHR | B_IMM | ALU_WRITE),
    row("SARI", SARI, 0, ALU_SAR | B_IMM | ALU_WRITE),
    row("CMPI", CMPI, 0, ALU_SUB | B_IMM | FLAGS_WE | LAST),
    // память данных
    row("LOAD шаг 0", LOAD, 0, ADDR_CALC),
    row("LOAD шаг 1", LOAD, 1, WB_MEM | REG_WE | LAST),
    row("STORE шаг 0", STORE, 0, ADDR_CALC),
    row("STORE шаг 1", STORE, 1, RS2_RD | MEM_WE | LAST),
    // переходы
    row("JUMP", JUMP, 0, JUMP_REL | COND_ALWAYS),
    row("BEQ", BEQ, 0, JUMP_REL | COND_Z1),
    row("BNE", BNE, 0, JUMP_REL | COND_Z0),
    row("BLTU", BLTU, 0, JUMP_REL | COND_C1),
    row("BGEU", BGEU, 0, JUMP_REL | COND_C0),
    row("JAL", JAL, 0, WB_PC | REG_WE | JUMP_REL | COND_ALWAYS),
    row("JR", JR, 0, PC_WE | PC_REG | COND_ALWAYS | LAST),
    row("HIGH_RES_TICK", HRT, 0, WB_CAPT | REG_WE | FLAGS_NOCAP | FLAGS_WE | LAST),
];

fn listed(opcode: u8, step: u8) -> bool {
    ROWS.iter().any(|r| r.opcode == opcode && r.step == step)
}

macro_rules! new_rom {
    ($runtime:ident, $rom:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/micro_rom.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $rom = $runtime.create_model_simple::<MicroRom>()?;
    };
}

macro_rules! word {
    ($rom:expr, $opcode:expr, $step:expr) => {{
        $rom.opcode = $opcode;
        $rom.step = $step;
        $rom.eval();
        $rom.ctrl
    }};
}

// Проверяет, что бит(ы) mask установлены ровно в тех (opcode, шаг), которые перечислены в allowed
macro_rules! check_signal_only_in {
    ($rom:expr, $mask:expr, $allowed:expr, $what:expr) => {{
        let mut failures = Vec::new();
        for opcode in 0..64u8 {
            for step in 0..16u8 {
                let w = word!($rom, opcode, step);
                let is_set = w & $mask != 0;
                let should = $allowed.contains(&(opcode, step));
                if is_set != should {
                    failures.push(format!(
                        "opcode {opcode:#04x} шаг {step}: {} = {is_set}, ожидалось {should}",
                        $what
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }};
}

#[test]
#[snafu::report]
fn defined_steps_match_isa_table() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    let mut failures = Vec::new();
    for r in ROWS {
        let w = word!(rom, r.opcode, r.step);
        if w != r.word {
            failures.push(format!(
                "{}: opcode {:#04x} шаг {}: ожидалось {:#08x} ({:022b}), получено {:#08x} ({:022b})",
                r.name, r.opcode, r.step, r.word, r.word, w, w
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    Ok(())
}

#[test]
#[snafu::report]
fn unknown_opcodes_and_steps_halt() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    let mut failures = Vec::new();
    for opcode in 0..64u8 {
        for step in 0..16u8 {
            if listed(opcode, step) {
                continue;
            }
            let w = word!(rom, opcode, step);
            if w != DEFAULT {
                failures.push(format!(
                    "opcode {opcode:#04x} шаг {step}: ожидалось {DEFAULT:#08x}, получено {w:#08x}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    Ok(())
}

#[test]
#[snafu::report]
fn last_is_set_only_on_the_final_step() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    let mut failures = Vec::new();
    for opcode in 0..64u8 {
        let steps: Vec<u8> = ROWS
            .iter()
            .filter(|r| r.opcode == opcode)
            .map(|r| r.step)
            .collect();
        if steps.is_empty() {
            continue;
        }
        let final_step = *steps.iter().max().unwrap();
        // шаги идут подряд, начиная с 0
        if steps.len() != usize::from(final_step) + 1 {
            failures.push(format!("opcode {opcode:#04x}: шаги {steps:?} не подряд"));
        }
        for step in 0..=final_step {
            let has_last = word!(rom, opcode, step) & LAST != 0;
            if has_last != (step == final_step) {
                failures.push(format!(
                    "opcode {opcode:#04x} шаг {step}: last = {has_last}, финальный шаг {final_step}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    Ok(())
}

#[test]
#[snafu::report]
fn immediate_forms_differ_from_register_forms_only_by_b_sel() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    let pairs = [
        (ADD, ADDI),
        (SUB, SUBI),
        (AND, ANDI),
        (OR, ORI),
        (XOR, XORI),
        (SHL, SHLI),
        (SHR, SHRI),
        (SAR, SARI),
        (CMP, CMPI),
    ];
    for (reg, imm) in pairs {
        assert_eq!(imm, reg + 0x10, "код формы с константой = код формы с регистром + 0x10");
        let w_reg = word!(rom, reg, 0);
        let w_imm = word!(rom, imm, 0);
        assert_eq!(w_reg & B_IMM, 0, "opcode {reg:#04x}: alu_b_sel должен быть 0");
        assert_eq!(w_imm, w_reg | B_IMM, "opcode {imm:#04x} отличается не только alu_b_sel");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn only_store_writes_memory() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    check_signal_only_in!(rom, MEM_WE, [(STORE, 1)], "mem_we");
    Ok(())
}

#[test]
#[snafu::report]
fn only_load_and_store_write_address_register() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    check_signal_only_in!(rom, ADDR_WE, [(LOAD, 0), (STORE, 0)], "addr_we");
    Ok(())
}

#[test]
#[snafu::report]
fn only_fetch_and_jumps_write_pc() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    check_signal_only_in!(
        rom,
        PC_WE,
        [
            (FETCH, 0),
            (JUMP, 0),
            (BEQ, 0),
            (BNE, 0),
            (BLTU, 0),
            (BGEU, 0),
            (JAL, 0),
            (JR, 0)
        ],
        "pc_we"
    );
    Ok(())
}

#[test]
#[snafu::report]
fn only_fetch_writes_ir() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    check_signal_only_in!(rom, IR_WE, [(FETCH, 0)], "ir_we");
    Ok(())
}

#[test]
#[snafu::report]
fn flags_are_written_only_by_alu_commands_and_high_res_tick() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    check_signal_only_in!(
        rom,
        FLAGS_WE,
        [
            (ADD, 0),
            (SUB, 0),
            (AND, 0),
            (OR, 0),
            (XOR, 0),
            (NOT, 0),
            (SHL, 0),
            (SHR, 0),
            (SAR, 0),
            (CMP, 0),
            (ADDI, 0),
            (SUBI, 0),
            (ANDI, 0),
            (ORI, 0),
            (XORI, 0),
            (SHLI, 0),
            (SHRI, 0),
            (SARI, 0),
            (CMPI, 0),
            (HRT, 0)
        ],
        "flags_we"
    );
    Ok(())
}

#[test]
#[snafu::report]
fn registers_are_written_only_where_expected() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    // CMP, CMPI, STORE, JUMP, условные переходы, JR, HALT и FETCH регистры не пишут
    check_signal_only_in!(
        rom,
        REG_WE,
        [
            (ADD, 0),
            (SUB, 0),
            (AND, 0),
            (OR, 0),
            (XOR, 0),
            (NOT, 0),
            (SHL, 0),
            (SHR, 0),
            (SAR, 0),
            (ADDI, 0),
            (SUBI, 0),
            (ANDI, 0),
            (ORI, 0),
            (XORI, 0),
            (SHLI, 0),
            (SHRI, 0),
            (SARI, 0),
            (LOAD, 1),
            (JAL, 0),
            (HRT, 0)
        ],
        "reg_we"
    );
    Ok(())
}

#[test]
#[snafu::report]
fn halt_is_set_only_for_halt_and_unknown_codes() -> Result<(), VerilatorError> {
    new_rom!(runtime, rom);
    let mut failures = Vec::new();
    for opcode in 0..64u8 {
        for step in 0..16u8 {
            let is_set = word!(rom, opcode, step) & HALT != 0;
            let should = !listed(opcode, step) || opcode == HALT_OP;
            if is_set != should {
                failures.push(format!(
                    "opcode {opcode:#04x} шаг {step}: halt = {is_set}, ожидалось {should}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    Ok(())
}
