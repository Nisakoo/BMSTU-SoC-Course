//! Ассемблер: строки программы превращаются в образы ROM и RAM (docs/ASM.md).
//!
//! Первый проход ведёт счётчики адреса ROM и RAM, раскладывает метки и константы.
//! Второй проход кодирует команды и слова данных; имена к этому времени известны все,
//! поэтому на метки можно ссылаться вперёд.

use std::collections::HashMap;

use crate::error::Error;
use crate::image::{Image, Memory, WORDS};
use crate::isa::{opcode, Op, Shape, Spec};
use crate::lexer::tokenize;
use crate::parser::{parse, Line, Operand, Statement, Value};

const IMM_MIN: i64 = -(1 << 17);
const IMM_MAX: i64 = (1 << 17) - 1;
const IMM_MASK: u32 = 0x3_FFFF;

/// Собирает текст программы.
pub fn assemble(text: &str) -> Result<Image, Error> {
    let tokens = tokenize(text)?;
    let lines = parse(&tokens)?;
    assemble_lines(&lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Text,
    Data,
}

impl Section {
    fn index(self) -> usize {
        match self {
            Section::Text => 0,
            Section::Data => 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Symbol {
    /// Константа из `.equ`.
    Const(i64),
    /// Метка: адрес в ROM (`.text`) или в RAM (`.data`).
    Label { section: Section, addr: i64 },
}

type Symbols = HashMap<String, Symbol>;

fn assemble_lines(lines: &[Line]) -> Result<Image, Error> {
    let (symbols, addrs) = layout(lines)?;

    let mut image = Image::default();
    for (line, &addr) in lines.iter().zip(&addrs) {
        match &line.statement {
            Some(Statement::Word(values)) => {
                for (offset, value) in values.iter().enumerate() {
                    let word = word32(value_of(value, &symbols, line.line)?, line.line)?;
                    put(&mut image.ram, addr + offset as i64, word, line.line)?;
                }
            }
            Some(Statement::Instruction { spec, operands }) => {
                let ctx = Ctx { symbols: &symbols, line: line.line };
                let word = ctx.encode(spec, operands, addr)?;
                put(&mut image.rom, addr, word, line.line)?;
            }
            _ => {}
        }
    }
    Ok(image)
}

/// Первый проход: таблица имён и адрес, с которого начинается команда или `.word` каждой строки.
fn layout(lines: &[Line]) -> Result<(Symbols, Vec<i64>), Error> {
    let mut symbols = Symbols::new();
    let mut addrs = Vec::with_capacity(lines.len());
    let mut counters = [0i64; 2];
    let mut section = Section::Text;

    for line in lines {
        let number = line.line;
        for label in &line.labels {
            let symbol = Symbol::Label { section, addr: counters[section.index()] };
            define(&mut symbols, label, symbol, number)?;
        }
        addrs.push(counters[section.index()]);

        match &line.statement {
            None => {}
            Some(Statement::Equ { name, value }) => define(&mut symbols, name, Symbol::Const(*value), number)?,
            Some(Statement::Text) => section = Section::Text,
            Some(Statement::Data) => section = Section::Data,
            Some(Statement::Org(value)) => {
                let addr = value_of(value, &symbols, number)
                    .map_err(|_| Error::new(number, ".org: имя должно быть определено выше"))?;
                if !(0..i64::from(WORDS)).contains(&addr) {
                    return Err(Error::new(number, format!(".org: адрес {addr} вне образа (0…{})", WORDS - 1)));
                }
                counters[section.index()] = addr;
            }
            Some(Statement::Word(values)) => {
                if section != Section::Data {
                    return Err(Error::new(number, ".word допустим только в .data"));
                }
                counters[section.index()] += values.len() as i64;
            }
            Some(Statement::Instruction { spec, .. }) => {
                if section != Section::Text {
                    return Err(Error::new(number, format!("команда {} допустима только в .text", spec.name)));
                }
                counters[section.index()] += 1;
            }
        }
    }
    Ok((symbols, addrs))
}

fn define(symbols: &mut Symbols, name: &str, symbol: Symbol, line: usize) -> Result<(), Error> {
    if symbols.insert(name.to_string(), symbol).is_some() {
        return Err(Error::new(line, format!("имя '{name}' уже определено")));
    }
    Ok(())
}

fn value_of(value: &Value, symbols: &Symbols, line: usize) -> Result<i64, Error> {
    match value {
        Value::Number(number) => Ok(*number),
        Value::Symbol(name) => match symbols.get(name) {
            Some(Symbol::Const(value)) => Ok(*value),
            Some(Symbol::Label { addr, .. }) => Ok(*addr),
            None => Err(Error::new(line, format!("неизвестное имя '{name}'"))),
        },
    }
}

/// Слово данных: от −2³¹ до 2³²−1.
fn word32(value: i64, line: usize) -> Result<u32, Error> {
    if (i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&value) {
        Ok(value as u32)
    } else {
        Err(Error::new(line, format!("число {value} не помещается в 32 бита")))
    }
}

fn put(memory: &mut Memory, addr: i64, word: u32, line: usize) -> Result<(), Error> {
    if !(0..i64::from(WORDS)).contains(&addr) {
        return Err(Error::new(line, format!("адрес {addr} выходит за размер образа ({WORDS} слов)")));
    }
    memory.set(addr as u32, word);
    Ok(())
}

fn reg_form(op: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (op << 26) | (rd << 22) | (rs1 << 18) | (rs2 << 14)
}

fn imm_form(op: u32, rd: u32, rs1: u32, imm: u32) -> u32 {
    (op << 26) | (rd << 22) | (rs1 << 18) | imm
}

/// Контекст кодирования одной команды.
struct Ctx<'a> {
    symbols: &'a Symbols,
    line: usize,
}

impl Ctx<'_> {
    fn error(&self, message: impl Into<String>) -> Error {
        Error::new(self.line, message)
    }

    fn internal(&self) -> Error {
        self.error("внутренняя ошибка: операнд не подходит команде (парсер должен был это отвергнуть)")
    }

    fn reg(&self, operand: &Operand) -> Result<u32, Error> {
        match operand {
            Operand::Reg(number) => Ok(u32::from(*number)),
            _ => Err(self.internal()),
        }
    }

    /// 18-битное поле `imm`.
    fn imm(&self, value: i64) -> Result<u32, Error> {
        if (IMM_MIN..=IMM_MAX).contains(&value) {
            Ok((value as u32) & IMM_MASK)
        } else {
            Err(self.error(format!("число {value} не помещается в 18 бит со знаком ({IMM_MIN}…{IMM_MAX})")))
        }
    }

    /// Число или значение имени (адрес метки, константа).
    fn literal(&self, operand: &Operand) -> Result<u32, Error> {
        match operand {
            Operand::Value(value) => self.imm(value_of(value, self.symbols, self.line)?),
            _ => Err(self.internal()),
        }
    }

    /// Адрес `imm(rs1)`: база и поле `imm`.
    fn memory(&self, operand: &Operand) -> Result<(u32, u32), Error> {
        match operand {
            Operand::Mem { offset, base } => {
                let imm = self.imm(value_of(offset, self.symbols, self.line)?)?;
                Ok((u32::from(*base), imm))
            }
            _ => Err(self.internal()),
        }
    }

    /// Цель перехода. Метка в `.text` превращается в смещение от следующей команды;
    /// число и константа `.equ` записываются в `imm` как есть.
    fn target(&self, operand: &Operand, addr: i64) -> Result<u32, Error> {
        match operand {
            Operand::Value(Value::Number(number)) => self.imm(*number),
            Operand::Value(Value::Symbol(name)) => match self.symbols.get(name) {
                Some(Symbol::Label { section: Section::Text, addr: target }) => self.imm(target - (addr + 1)),
                Some(Symbol::Label { section: Section::Data, .. }) => {
                    Err(self.error(format!("метка '{name}' лежит в .data, перейти на неё нельзя")))
                }
                Some(Symbol::Const(value)) => self.imm(*value),
                None => Err(self.error(format!("неизвестное имя '{name}'"))),
            },
            _ => Err(self.internal()),
        }
    }

    fn encode(&self, spec: &Spec, operands: &[Operand], addr: i64) -> Result<u32, Error> {
        match spec.op {
            // NOP = ADD R0, R0, R0
            Op::Nop => Ok(reg_form(opcode::ADD, 0, 0, 0)),
            // MOV rd, rs = ADD rd, rs, R0
            Op::Mov => Ok(reg_form(opcode::ADD, self.reg(&operands[0])?, self.reg(&operands[1])?, 0)),
            // LI rd, imm = ADDI rd, R0, imm
            Op::Li => Ok(imm_form(opcode::ADDI, self.reg(&operands[0])?, 0, self.literal(&operands[1])?)),
            Op::Real(code) => self.encode_real(code, spec.shape, operands, addr),
        }
    }

    fn encode_real(&self, code: u32, shape: Shape, ops: &[Operand], addr: i64) -> Result<u32, Error> {
        match shape {
            Shape::None => Ok(reg_form(code, 0, 0, 0)),
            Shape::RdRs1Rs2 => Ok(reg_form(code, self.reg(&ops[0])?, self.reg(&ops[1])?, self.reg(&ops[2])?)),
            Shape::RdRs1Imm => Ok(imm_form(code, self.reg(&ops[0])?, self.reg(&ops[1])?, self.literal(&ops[2])?)),
            Shape::RdRs1 => Ok(reg_form(code, self.reg(&ops[0])?, self.reg(&ops[1])?, 0)),
            Shape::Rs1Rs2 => Ok(reg_form(code, 0, self.reg(&ops[0])?, self.reg(&ops[1])?)),
            Shape::Rs1Imm => Ok(imm_form(code, 0, self.reg(&ops[0])?, self.literal(&ops[1])?)),
            Shape::RdMem => {
                let (base, imm) = self.memory(&ops[1])?;
                Ok(imm_form(code, self.reg(&ops[0])?, base, imm))
            }
            Shape::Imm => Ok(imm_form(code, 0, 0, self.target(&ops[0], addr)?)),
            Shape::RdImm => Ok(imm_form(code, self.reg(&ops[0])?, 0, self.target(&ops[1], addr)?)),
            Shape::Rs1 => Ok(reg_form(code, 0, self.reg(&ops[0])?, 0)),
            Shape::Rd => Ok(reg_form(code, self.reg(&ops[0])?, 0, 0)),
        }
    }
}
