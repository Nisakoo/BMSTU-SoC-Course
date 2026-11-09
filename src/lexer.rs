//! Лексер: превращает текст программы в токены.
//!
//! Правила (см. docs/ASM.md):
//! - комментарий начинается с `;` и идёт до конца строки;
//! - регистр букв не важен: имена и директивы приводятся к верхнему регистру;
//! - числа: десятичные (можно с `-`), `0x`, `0b`, символ `'a'`;
//! - выражений нет, `-` бывает только знаком числа;
//! - каждая строка текста заканчивается токеном `Newline`, даже пустая и последняя.

use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// Мнемоника, регистр, метка или имя константы, в верхнем регистре.
    Ident(String),
    /// Директива без точки, в верхнем регистре: `.equ` даёт `Directive("EQU")`.
    Directive(String),
    Number(i64),
    Comma,
    Colon,
    LParen,
    RParen,
    Newline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    /// Номер строки, с 1.
    pub line: usize,
    /// Номер символа в строке, с 1.
    pub col: usize,
}

/// Разбивает весь текст программы на токены.
pub fn tokenize(text: &str) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        lex_line(line, line_no, &mut tokens)?;
        tokens.push(Token {
            kind: TokenKind::Newline,
            line: line_no,
            col: line.chars().count() + 1,
        });
    }
    Ok(tokens)
}

fn lex_line(line: &str, line_no: usize, out: &mut Vec<Token>) -> Result<(), Error> {
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        let c = chars[pos];
        let col = pos + 1;
        let fail = |message: String| Error::new(line_no, format!("столбец {col}: {message}"));
        let mut push = |kind: TokenKind| out.push(Token { kind, line: line_no, col });

        match c {
            ' ' | '\t' => pos += 1,
            ';' => break,
            ',' => {
                push(TokenKind::Comma);
                pos += 1;
            }
            ':' => {
                push(TokenKind::Colon);
                pos += 1;
            }
            '(' => {
                push(TokenKind::LParen);
                pos += 1;
            }
            ')' => {
                push(TokenKind::RParen);
                pos += 1;
            }
            '.' => {
                let start = pos + 1;
                if start >= chars.len() || !is_ident_start(chars[start]) {
                    return Err(fail("после '.' должно быть имя директивы".to_string()));
                }
                let end = take_while(&chars, start, is_ident_char);
                let name: String = chars[start..end].iter().collect();
                push(TokenKind::Directive(name.to_ascii_uppercase()));
                pos = end;
            }
            '\'' => {
                if chars.len() > pos + 2 && chars[pos + 2] == '\'' {
                    push(TokenKind::Number(chars[pos + 1] as i64));
                    pos += 3;
                } else {
                    return Err(fail("символ должен быть записан как 'a'".to_string()));
                }
            }
            '-' | '0'..='9' => {
                let negative = c == '-';
                let start = if negative { pos + 1 } else { pos };
                if start >= chars.len() || !chars[start].is_ascii_digit() {
                    return Err(fail("после '-' должна идти цифра".to_string()));
                }
                let end = take_while(&chars, start, is_ident_char);
                let word: String = chars[start..end].iter().collect();
                let Some(value) = parse_number(&word) else {
                    return Err(fail(format!("неверное число '{word}'")));
                };
                push(TokenKind::Number(if negative { -value } else { value }));
                pos = end;
            }
            c if is_ident_start(c) => {
                let end = take_while(&chars, pos, is_ident_char);
                let word: String = chars[pos..end].iter().collect();
                push(TokenKind::Ident(word.to_ascii_uppercase()));
                pos = end;
            }
            other => return Err(fail(format!("неожиданный символ '{other}'"))),
        }
    }
    Ok(())
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Индекс первого символа, начиная с `start`, который не подходит под `pred`.
fn take_while(chars: &[char], start: usize, pred: fn(char) -> bool) -> usize {
    let mut end = start;
    while end < chars.len() && pred(chars[end]) {
        end += 1;
    }
    end
}

/// Число без знака: `10`, `0x3F`, `0b1010`.
fn parse_number(word: &str) -> Option<i64> {
    let lower = word.to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()
    } else if let Some(bin) = lower.strip_prefix("0b") {
        i64::from_str_radix(bin, 2).ok()
    } else {
        lower.parse::<i64>().ok()
    }
}
