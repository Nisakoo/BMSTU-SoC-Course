//! Таблица команд: коды операций и формы операндов (docs/ISA.md, docs/ASM.md).
//!
//! Имена в таблице написаны в верхнем регистре: лексер приводит текст к верхнему.

/// Коды операций (`IR[31:26]`). `FETCH` (0x3F) служебный и в программах не используется.
pub mod opcode {
    pub const HALT: u32 = 0x00;
    pub const ADD: u32 = 0x01;
    pub const SUB: u32 = 0x02;
    pub const AND: u32 = 0x03;
    pub const OR: u32 = 0x04;
    pub const XOR: u32 = 0x05;
    pub const NOT: u32 = 0x06;
    pub const SHL: u32 = 0x07;
    pub const SHR: u32 = 0x08;
    pub const SAR: u32 = 0x09;
    pub const CMP: u32 = 0x0A;

    pub const ADDI: u32 = 0x11;
    pub const SUBI: u32 = 0x12;
    pub const ANDI: u32 = 0x13;
    pub const ORI: u32 = 0x14;
    pub const XORI: u32 = 0x15;
    pub const SHLI: u32 = 0x17;
    pub const SHRI: u32 = 0x18;
    pub const SARI: u32 = 0x19;
    pub const CMPI: u32 = 0x1A;

    pub const LOAD: u32 = 0x20;
    pub const STORE: u32 = 0x21;

    pub const JUMP: u32 = 0x30;
    pub const BEQ: u32 = 0x31;
    pub const BNE: u32 = 0x32;
    pub const BLTU: u32 = 0x33;
    pub const BGEU: u32 = 0x34;
    pub const JAL: u32 = 0x35;
    pub const JR: u32 = 0x36;

    pub const HIGH_RES_TICK: u32 = 0x38;
}

/// Вид одного операнда.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    /// Регистр `R0`–`R15`.
    Reg,
    /// Число или имя (метка, константа `.equ`).
    Imm,
    /// Адрес `imm(rs1)`.
    Mem,
}

/// Набор операндов команды.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// без операндов
    None,
    /// `rd, rs1, rs2`
    RdRs1Rs2,
    /// `rd, rs1, imm`
    RdRs1Imm,
    /// `rd, rs1`
    RdRs1,
    /// `rs1, rs2`
    Rs1Rs2,
    /// `rs1, imm`
    Rs1Imm,
    /// `rd, imm(rs1)`
    RdMem,
    /// `imm` (метка или число)
    Imm,
    /// `rd, imm`
    RdImm,
    /// `rs1`
    Rs1,
    /// `rd`
    Rd,
}

impl Shape {
    /// Виды операндов по порядку.
    pub fn operands(self) -> &'static [OperandKind] {
        use OperandKind::{Imm, Mem, Reg};
        match self {
            Shape::None => &[],
            Shape::RdRs1Rs2 => &[Reg, Reg, Reg],
            Shape::RdRs1Imm => &[Reg, Reg, Imm],
            Shape::RdRs1 => &[Reg, Reg],
            Shape::Rs1Rs2 => &[Reg, Reg],
            Shape::Rs1Imm => &[Reg, Imm],
            Shape::RdMem => &[Reg, Mem],
            Shape::Imm => &[Imm],
            Shape::RdImm => &[Reg, Imm],
            Shape::Rs1 => &[Reg],
            Shape::Rd => &[Reg],
        }
    }
}

/// Что делает команда при сборке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Настоящая команда с этим кодом операции.
    Real(u32),
    /// `NOP` → `ADD R0, R0, R0`
    Nop,
    /// `MOV rd, rs` → `ADD rd, rs, R0`
    Mov,
    /// `LI rd, imm` → `ADDI rd, R0, imm`
    Li,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub name: &'static str,
    pub op: Op,
    pub shape: Shape,
}

const fn real(name: &'static str, code: u32, shape: Shape) -> Spec {
    Spec { name, op: Op::Real(code), shape }
}

pub static INSTRUCTIONS: &[Spec] = &[
    real("HALT", opcode::HALT, Shape::None),
    real("ADD", opcode::ADD, Shape::RdRs1Rs2),
    real("SUB", opcode::SUB, Shape::RdRs1Rs2),
    real("AND", opcode::AND, Shape::RdRs1Rs2),
    real("OR", opcode::OR, Shape::RdRs1Rs2),
    real("XOR", opcode::XOR, Shape::RdRs1Rs2),
    real("NOT", opcode::NOT, Shape::RdRs1),
    real("SHL", opcode::SHL, Shape::RdRs1Rs2),
    real("SHR", opcode::SHR, Shape::RdRs1Rs2),
    real("SAR", opcode::SAR, Shape::RdRs1Rs2),
    real("CMP", opcode::CMP, Shape::Rs1Rs2),
    real("ADDI", opcode::ADDI, Shape::RdRs1Imm),
    real("SUBI", opcode::SUBI, Shape::RdRs1Imm),
    real("ANDI", opcode::ANDI, Shape::RdRs1Imm),
    real("ORI", opcode::ORI, Shape::RdRs1Imm),
    real("XORI", opcode::XORI, Shape::RdRs1Imm),
    real("SHLI", opcode::SHLI, Shape::RdRs1Imm),
    real("SHRI", opcode::SHRI, Shape::RdRs1Imm),
    real("SARI", opcode::SARI, Shape::RdRs1Imm),
    real("CMPI", opcode::CMPI, Shape::Rs1Imm),
    real("LOAD", opcode::LOAD, Shape::RdMem),
    real("STORE", opcode::STORE, Shape::RdMem),
    real("JUMP", opcode::JUMP, Shape::Imm),
    real("BEQ", opcode::BEQ, Shape::Imm),
    real("BNE", opcode::BNE, Shape::Imm),
    real("BLTU", opcode::BLTU, Shape::Imm),
    real("BGEU", opcode::BGEU, Shape::Imm),
    real("JAL", opcode::JAL, Shape::RdImm),
    real("JR", opcode::JR, Shape::Rs1),
    real("HIGH_RES_TICK", opcode::HIGH_RES_TICK, Shape::Rd),
    // псевдокоманды
    Spec { name: "NOP", op: Op::Nop, shape: Shape::None },
    Spec { name: "MOV", op: Op::Mov, shape: Shape::RdRs1 },
    Spec { name: "LI", op: Op::Li, shape: Shape::RdImm },
];

/// Команда по имени в верхнем регистре.
pub fn find(name: &str) -> Option<&'static Spec> {
    INSTRUCTIONS.iter().find(|spec| spec.name == name)
}

/// Номер регистра по имени в верхнем регистре: `R0`–`R15`.
/// Ведущие нули не допускаются: `R01` не регистр.
pub fn register(name: &str) -> Option<u8> {
    let digits = name.strip_prefix('R')?;
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number: u8 = digits.parse().ok()?;
    (number <= 15).then_some(number)
}
