//! Парсер: превращает токены в строки программы.
//!
//! Строка — это метки (может быть несколько) и необязательная команда или директива.
//! Парсер проверяет форму: нужные операнды команды и их виды (регистр, число или имя,
//! адрес `imm(rs1)`). Значения имён (метки, `.equ`) он не вычисляет, это делает ассемблер.
//!
//! Пустые строки и строки с одним комментарием в результат не попадают.

use crate::error::Error;
use crate::isa::{self, OperandKind, Spec};
use crate::lexer::{Token, TokenKind};

/// Число или имя (метка, константа `.equ`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(i64),
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Reg(u8),
    Value(Value),
    /// `offset(base)`
    Mem { offset: Value, base: u8 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Instruction { spec: &'static Spec, operands: Vec<Operand> },
    /// `.equ имя, число`
    Equ { name: String, value: i64 },
    Text,
    Data,
    /// `.word v1, v2, ...`
    Word(Vec<Value>),
    /// `.org адрес`
    Org(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Номер строки в исходном тексте, с 1.
    pub line: usize,
    pub labels: Vec<String>,
    pub statement: Option<Statement>,
}

/// Разбирает токены (результат `lexer::tokenize`) в строки программы.
pub fn parse(tokens: &[Token]) -> Result<Vec<Line>, Error> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == TokenKind::Newline {
            if let Some(line) = parse_line(&tokens[start..index], token)? {
                lines.push(line);
            }
            start = index + 1;
        }
    }
    Ok(lines)
}

fn fail(token: &Token, message: impl AsRef<str>) -> Error {
    Error::new(token.line, format!("столбец {}: {}", token.col, message.as_ref()))
}

/// Токены одной строки без `Newline`; `end` — `Newline` этой строки (для позиции ошибок).
struct Cursor<'a> {
    tokens: &'a [Token],
    pos: usize,
    end: &'a Token,
}

impl<'a> Cursor<'a> {
    fn peek(&self, ahead: usize) -> Option<&'a Token> {
        self.tokens.get(self.pos + ahead)
    }

    fn advance(&mut self) -> Option<&'a Token> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Токен, на котором стоим; в конце строки — `Newline`.
    fn here(&self) -> &'a Token {
        self.peek(0).unwrap_or(self.end)
    }

    fn expect_end(&self) -> Result<(), Error> {
        match self.peek(0) {
            None => Ok(()),
            Some(token) => Err(fail(token, "ожидался конец строки")),
        }
    }
}

fn parse_line(tokens: &[Token], end: &Token) -> Result<Option<Line>, Error> {
    let mut cur = Cursor { tokens, pos: 0, end };
    let mut labels = Vec::new();

    while let (Some(first), Some(second)) = (cur.peek(0), cur.peek(1)) {
        match (&first.kind, &second.kind) {
            (TokenKind::Ident(name), TokenKind::Colon) => {
                if isa::register(name).is_some() {
                    return Err(fail(first, format!("метка не может называться как регистр '{name}'")));
                }
                labels.push(name.clone());
                cur.pos += 2;
            }
            _ => break,
        }
    }

    let statement = if cur.at_end() {
        None
    } else {
        let statement = parse_statement(&mut cur)?;
        cur.expect_end()?;
        Some(statement)
    };

    if labels.is_empty() && statement.is_none() {
        return Ok(None);
    }
    Ok(Some(Line { line: end.line, labels, statement }))
}

fn parse_statement(cur: &mut Cursor) -> Result<Statement, Error> {
    let Some(token) = cur.advance() else {
        return Err(fail(cur.end, "ожидалась команда или директива"));
    };
    match &token.kind {
        TokenKind::Ident(name) => parse_instruction(cur, token, name),
        TokenKind::Directive(name) => parse_directive(cur, token, name),
        _ => Err(fail(token, "ожидалась команда или директива")),
    }
}

// ---------- команды ----------

fn parse_instruction(cur: &mut Cursor, token: &Token, name: &str) -> Result<Statement, Error> {
    let Some(spec) = isa::find(name) else {
        return Err(fail(token, format!("неизвестная команда '{name}'")));
    };
    let operands = parse_operands(cur)?;
    if let Some(extra) = cur.peek(0) {
        return Err(fail(extra, "ожидалась ',' или конец строки"));
    }

    let expected = spec.shape.operands();
    if operands.len() != expected.len() {
        return Err(fail(
            token,
            format!("{name} ожидает операндов: {}, получено: {}", expected.len(), operands.len()),
        ));
    }
    for (index, ((operand, at), kind)) in operands.iter().zip(expected).enumerate() {
        let fits = matches!(
            (kind, operand),
            (OperandKind::Reg, Operand::Reg(_))
                | (OperandKind::Imm, Operand::Value(_))
                | (OperandKind::Mem, Operand::Mem { .. })
        );
        if !fits {
            let wanted = match kind {
                OperandKind::Reg => "регистр",
                OperandKind::Imm => "число или имя",
                OperandKind::Mem => "адрес вида imm(rs1)",
            };
            return Err(fail(at, format!("операнд {} команды {name}: ожидалось {wanted}", index + 1)));
        }
    }

    Ok(Statement::Instruction {
        spec,
        operands: operands.into_iter().map(|(operand, _)| operand).collect(),
    })
}

/// Операнды через запятую до конца строки; у каждого запоминается токен для позиции ошибки.
fn parse_operands<'a>(cur: &mut Cursor<'a>) -> Result<Vec<(Operand, &'a Token)>, Error> {
    let mut operands = Vec::new();
    if cur.at_end() {
        return Ok(operands);
    }
    loop {
        operands.push(parse_operand(cur)?);
        match cur.peek(0) {
            Some(token) if token.kind == TokenKind::Comma => {
                cur.advance();
            }
            _ => return Ok(operands),
        }
    }
}

fn parse_operand<'a>(cur: &mut Cursor<'a>) -> Result<(Operand, &'a Token), Error> {
    let Some(token) = cur.advance() else {
        return Err(fail(cur.end, "ожидался операнд"));
    };
    let value = match &token.kind {
        TokenKind::Number(number) => Value::Number(*number),
        TokenKind::Ident(name) => match isa::register(name) {
            Some(number) => return Ok((Operand::Reg(number), token)),
            None => Value::Symbol(name.clone()),
        },
        _ => return Err(fail(token, "ожидался операнд")),
    };

    // `imm(rs1)`
    if matches!(cur.peek(0), Some(next) if next.kind == TokenKind::LParen) {
        cur.advance();
        let base = match cur.advance() {
            Some(Token { kind: TokenKind::Ident(name), .. }) => isa::register(name),
            _ => None,
        };
        let Some(base) = base else {
            return Err(fail(cur.peek(0).unwrap_or(cur.end), "в скобках ожидался регистр"));
        };
        match cur.advance() {
            Some(Token { kind: TokenKind::RParen, .. }) => {}
            _ => return Err(fail(cur.here(), "ожидалась ')'")),
        }
        return Ok((Operand::Mem { offset: value, base }, token));
    }
    Ok((Operand::Value(value), token))
}

// ---------- директивы ----------

fn parse_directive(cur: &mut Cursor, token: &Token, name: &str) -> Result<Statement, Error> {
    match name {
        "TEXT" => Ok(Statement::Text),
        "DATA" => Ok(Statement::Data),
        "EQU" => parse_equ(cur),
        "WORD" => {
            let mut values = vec![parse_value(cur)?];
            while matches!(cur.peek(0), Some(next) if next.kind == TokenKind::Comma) {
                cur.advance();
                values.push(parse_value(cur)?);
            }
            Ok(Statement::Word(values))
        }
        "ORG" => Ok(Statement::Org(parse_value(cur)?)),
        _ => Err(fail(token, format!("неизвестная директива '.{}'", name.to_ascii_lowercase()))),
    }
}

fn parse_equ(cur: &mut Cursor) -> Result<Statement, Error> {
    let name = match cur.advance() {
        Some(Token { kind: TokenKind::Ident(name), .. }) if isa::register(name).is_none() => name.clone(),
        Some(token) => return Err(fail(token, ".equ: ожидалось имя константы (не регистр)")),
        None => return Err(fail(cur.end, ".equ: ожидалось имя константы")),
    };
    match cur.advance() {
        Some(Token { kind: TokenKind::Comma, .. }) => {}
        _ => return Err(fail(cur.here(), ".equ: после имени ожидалась ','")),
    }
    match cur.advance() {
        Some(Token { kind: TokenKind::Number(value), .. }) => Ok(Statement::Equ { name, value: *value }),
        Some(token) => Err(fail(token, ".equ: значение должно быть числом")),
        None => Err(fail(cur.end, ".equ: ожидалось значение")),
    }
}

/// Число или имя (не регистр).
fn parse_value(cur: &mut Cursor) -> Result<Value, Error> {
    match cur.advance() {
        Some(Token { kind: TokenKind::Number(value), .. }) => Ok(Value::Number(*value)),
        Some(Token { kind: TokenKind::Ident(name), .. }) if isa::register(name).is_none() => {
            Ok(Value::Symbol(name.clone()))
        }
        Some(token) => Err(fail(token, "ожидалось число или имя")),
        None => Err(fail(cur.end, "ожидалось число или имя")),
    }
}
