use soc::lexer::{tokenize, Token, TokenKind};
use TokenKind::{Colon, Comma, Directive, LParen, Newline, Number, RParen};

fn ident(s: &str) -> TokenKind {
    TokenKind::Ident(s.to_string())
}

fn directive(s: &str) -> TokenKind {
    Directive(s.to_string())
}

fn kinds(text: &str) -> Vec<TokenKind> {
    tokenize(text)
        .unwrap_or_else(|e| panic!("{text:?}: {e}"))
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[test]
fn empty_text_has_no_tokens() {
    assert_eq!(kinds(""), vec![]);
}

#[test]
fn every_line_ends_with_newline_even_the_last_and_empty_ones() {
    assert_eq!(kinds("\n"), vec![Newline]);
    assert_eq!(kinds("\n\n"), vec![Newline, Newline]);
    assert_eq!(kinds("HALT"), vec![ident("HALT"), Newline]);
    assert_eq!(kinds("HALT\n"), vec![ident("HALT"), Newline]);
    assert_eq!(
        kinds("NOP\n\nHALT"),
        vec![ident("NOP"), Newline, Newline, ident("HALT"), Newline]
    );
}

#[test]
fn instruction_with_three_registers() {
    assert_eq!(
        kinds("ADD R1, R2, R3"),
        vec![ident("ADD"), ident("R1"), Comma, ident("R2"), Comma, ident("R3"), Newline]
    );
}

#[test]
fn names_and_directives_are_upper_cased() {
    assert_eq!(kinds("add r1, Loop"), vec![ident("ADD"), ident("R1"), Comma, ident("LOOP"), Newline]);
    assert_eq!(kinds(".Equ led, 1"), vec![directive("EQU"), ident("LED"), Comma, Number(1), Newline]);
    assert_eq!(kinds("arr_size"), vec![ident("ARR_SIZE"), Newline]);
    assert_eq!(kinds("_x1"), vec![ident("_X1"), Newline]);
}

#[test]
fn labels() {
    assert_eq!(kinds("loop:"), vec![ident("LOOP"), Colon, Newline]);
    assert_eq!(
        kinds("loop: BNE loop"),
        vec![ident("LOOP"), Colon, ident("BNE"), ident("LOOP"), Newline]
    );
    assert_eq!(kinds("loop :"), vec![ident("LOOP"), Colon, Newline]);
}

#[test]
fn memory_operand() {
    assert_eq!(
        kinds("LOAD R1, 4(R2)"),
        vec![ident("LOAD"), ident("R1"), Comma, Number(4), LParen, ident("R2"), RParen, Newline]
    );
    assert_eq!(
        kinds("STORE R1, led(R0)"),
        vec![ident("STORE"), ident("R1"), Comma, ident("LED"), LParen, ident("R0"), RParen, Newline]
    );
    assert_eq!(
        kinds("LOAD R1,-4(R2)"),
        vec![ident("LOAD"), ident("R1"), Comma, Number(-4), LParen, ident("R2"), RParen, Newline]
    );
}

#[test]
fn directives() {
    assert_eq!(kinds(".text"), vec![directive("TEXT"), Newline]);
    assert_eq!(kinds(".data"), vec![directive("DATA"), Newline]);
    assert_eq!(kinds(".org 16"), vec![directive("ORG"), Number(16), Newline]);
    assert_eq!(
        kinds(".word 1, 2, 3"),
        vec![directive("WORD"), Number(1), Comma, Number(2), Comma, Number(3), Newline]
    );
    assert_eq!(
        kinds("arr: .word 1"),
        vec![ident("ARR"), Colon, directive("WORD"), Number(1), Newline]
    );
}

#[test]
fn numbers() {
    let cases: [(&str, i64); 17] = [
        ("0", 0),
        ("10", 10),
        ("007", 7),
        ("-976", -976),
        ("-0", 0),
        ("0x3F", 0x3F),
        ("0x3f", 0x3F),
        ("0XFF", 0xFF),
        ("0xCAFEBABE", 0xCAFE_BABE),
        ("-0x10", -16),
        ("0b1010", 10),
        ("0B11", 3),
        ("-0b1", -1),
        ("'a'", 97),
        ("'0'", 48),
        ("' '", 32),
        ("'я'", 1103),
    ];
    for (text, value) in cases {
        assert_eq!(kinds(text), vec![Number(value), Newline], "{text}");
    }
}

#[test]
fn comments_are_skipped() {
    assert_eq!(kinds("; только комментарий"), vec![Newline]);
    assert_eq!(kinds("   ; с отступом"), vec![Newline]);
    assert_eq!(kinds("HALT ; остановка"), vec![ident("HALT"), Newline]);
    assert_eq!(kinds("HALT;без пробела"), vec![ident("HALT"), Newline]);
    assert_eq!(kinds("ADD R1, R2, R3 ; , : ( ) @ ' ."), vec![
        ident("ADD"), ident("R1"), Comma, ident("R2"), Comma, ident("R3"), Newline
    ]);
    assert_eq!(kinds("';'"), vec![Number(59), Newline], "';' в кавычках — символ, а не комментарий");
}

#[test]
fn spaces_tabs_and_crlf() {
    assert_eq!(kinds("\tADD\tR1,R2,R3"), kinds("ADD R1, R2, R3"));
    assert_eq!(kinds("   ADD   R1  ,  R2 ,R3   "), kinds("ADD R1, R2, R3"));
    assert_eq!(kinds("NOP\r\nHALT\r\n"), kinds("NOP\nHALT\n"));
}

#[test]
fn lines_and_columns() {
    let tokens = tokenize("  ADD R1, R2\nloop: HALT").unwrap();
    let positions: Vec<(usize, usize)> = tokens.iter().map(|t| (t.line, t.col)).collect();
    assert_eq!(
        positions,
        vec![
            (1, 3),  // ADD
            (1, 7),  // R1
            (1, 9),  // ,
            (1, 11), // R2
            (1, 13), // Newline
            (2, 1),  // LOOP
            (2, 5),  // :
            (2, 7),  // HALT
            (2, 11), // Newline
        ]
    );
}

#[test]
fn columns_count_characters_not_bytes() {
    let tokens = tokenize("; я\nADD").unwrap();
    assert_eq!(tokens[0], Token { kind: Newline, line: 1, col: 4 });
    assert_eq!(tokens[1].col, 1);
}

#[test]
fn whole_program_from_the_example() {
    let text = "\
.equ LED, -976
        .data
arr:    .word 1, 'a'
        .text
        LI    R1, 8     ; счётчик
loop:   STORE R1, LED(R0)
        SUBI  R1, R1, 1
        BNE   loop
        HALT
";
    let tokens = tokenize(text).unwrap();
    // 9 строк — 9 токенов Newline
    assert_eq!(tokens.iter().filter(|t| t.kind == Newline).count(), 9);
    assert_eq!(tokens.last().unwrap().line, 9);
}

#[test]
fn errors() {
    let cases: [(&str, &str); 13] = [
        ("ADD R1, @", "неожиданный символ '@'"),
        ("ADD R1, #5", "неожиданный символ '#'"),
        ("ADD Ы", "неожиданный символ 'Ы'"),
        ("12abc", "неверное число '12abc'"),
        ("0x", "неверное число '0x'"),
        ("0xZZ", "неверное число '0xZZ'"),
        ("0b102", "неверное число '0b102'"),
        ("1_000", "неверное число '1_000'"),
        ("99999999999999999999", "неверное число"),
        ("-", "после '-' должна идти цифра"),
        ("- 5", "после '-' должна идти цифра"),
        ("'ab'", "символ должен быть записан как 'a'"),
        (".", "после '.' должно быть имя директивы"),
    ];
    for (text, message) in cases {
        let error = tokenize(text).expect_err(text);
        assert_eq!(error.line, 1, "{text}");
        assert!(error.message.contains(message), "{text}: {}", error.message);
    }
}

#[test]
fn minus_is_only_a_sign_so_expressions_split_into_two_tokens() {
    // выражений в языке нет: такую запись отвергнет парсер (нет запятой между операндами)
    assert_eq!(kinds("R1-1"), vec![ident("R1"), Number(-1), Newline]);
    assert_eq!(kinds("arr_size-1"), vec![ident("ARR_SIZE"), Number(-1), Newline]);
}

#[test]
fn more_error_shapes() {
    assert!(tokenize(". equ").is_err());
    assert!(tokenize(".1").is_err());
    assert!(tokenize("'").is_err());
    assert!(tokenize("''").is_err());
    assert!(tokenize("'a").is_err());
    assert!(tokenize("'abc").is_err());
    assert!(tokenize("'a b'").is_err());
}

#[test]
fn error_has_line_and_column() {
    let error = tokenize("NOP\nHALT\n  ADD R1, @").unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.message.starts_with("столбец 11:"), "{}", error.message);
    assert_eq!(error.to_string(), format!("строка 3: {}", error.message));
}
