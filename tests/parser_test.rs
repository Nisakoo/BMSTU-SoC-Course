use soc::lexer::tokenize;
use soc::parser::{parse, Line, Operand, Statement, Value};
use std::collections::HashSet;

fn lines(text: &str) -> Vec<Line> {
    let tokens = tokenize(text).unwrap_or_else(|e| panic!("{text:?}: лексер: {e}"));
    parse(&tokens).unwrap_or_else(|e| panic!("{text:?}: {e}"))
}

fn error(text: &str) -> soc::error::Error {
    let tokens = tokenize(text).unwrap_or_else(|e| panic!("{text:?}: лексер: {e}"));
    parse(&tokens).expect_err(text)
}

/// Единственная команда в тексте: имя и операнды.
fn instruction(text: &str) -> (&'static str, Vec<Operand>) {
    let mut parsed = lines(text);
    assert_eq!(parsed.len(), 1, "{text}");
    match parsed.remove(0).statement {
        Some(Statement::Instruction { spec, operands }) => (spec.name, operands),
        other => panic!("{text}: не команда: {other:?}"),
    }
}

fn statement(text: &str) -> Statement {
    let mut parsed = lines(text);
    assert_eq!(parsed.len(), 1, "{text}");
    parsed.remove(0).statement.unwrap_or_else(|| panic!("{text}: нет команды"))
}

fn reg(n: u8) -> Operand {
    Operand::Reg(n)
}
fn num(n: i64) -> Operand {
    Operand::Value(Value::Number(n))
}
fn sym(s: &str) -> Operand {
    Operand::Value(Value::Symbol(s.to_string()))
}
fn mem(offset: Value, base: u8) -> Operand {
    Operand::Mem { offset, base }
}

#[test]
fn empty_and_comment_lines_give_nothing() {
    assert_eq!(lines(""), vec![]);
    assert_eq!(lines("\n\n   \n"), vec![]);
    assert_eq!(lines("; комментарий\n   ; ещё\n"), vec![]);
}

#[test]
fn every_instruction_shape() {
    let cases: Vec<(&str, &str, Vec<Operand>)> = vec![
        ("HALT", "HALT", vec![]),
        ("ADD R1, R2, R3", "ADD", vec![reg(1), reg(2), reg(3)]),
        ("SAR R15, R0, R7", "SAR", vec![reg(15), reg(0), reg(7)]),
        ("NOT R1, R2", "NOT", vec![reg(1), reg(2)]),
        ("CMP R3, R4", "CMP", vec![reg(3), reg(4)]),
        ("ADDI R1, R2, 5", "ADDI", vec![reg(1), reg(2), num(5)]),
        ("SUBI R1, R1, -1", "SUBI", vec![reg(1), reg(1), num(-1)]),
        ("ANDI R1, R2, mask", "ANDI", vec![reg(1), reg(2), sym("MASK")]),
        ("CMPI R1, 9", "CMPI", vec![reg(1), num(9)]),
        ("CMPI R1, last", "CMPI", vec![reg(1), sym("LAST")]),
        ("LOAD R1, 4(R2)", "LOAD", vec![reg(1), mem(Value::Number(4), 2)]),
        ("STORE R1, led(R0)", "STORE", vec![reg(1), mem(Value::Symbol("LED".into()), 0)]),
        ("JUMP loop", "JUMP", vec![sym("LOOP")]),
        ("BNE loop", "BNE", vec![sym("LOOP")]),
        ("BEQ -3", "BEQ", vec![num(-3)]),
        ("JAL R15, func", "JAL", vec![reg(15), sym("FUNC")]),
        ("JR R15", "JR", vec![reg(15)]),
        ("HIGH_RES_TICK R1", "HIGH_RES_TICK", vec![reg(1)]),
    ];
    for (text, name, operands) in cases {
        assert_eq!(instruction(text), (name, operands), "{text}");
    }
}

#[test]
fn pseudo_instructions_parse_like_normal_ones() {
    assert_eq!(instruction("NOP"), ("NOP", vec![]));
    assert_eq!(instruction("MOV R1, R2"), ("MOV", vec![reg(1), reg(2)]));
    assert_eq!(instruction("LI R1, 5"), ("LI", vec![reg(1), num(5)]));
    assert_eq!(instruction("LI R1, big"), ("LI", vec![reg(1), sym("BIG")]));
}

#[test]
fn case_does_not_matter() {
    assert_eq!(instruction("add r1, R2, r3"), instruction("ADD R1, R2, R3"));
    assert_eq!(instruction("Load r1, Arr(r2)"), instruction("LOAD R1, ARR(R2)"));
}

#[test]
fn whitespace_does_not_matter() {
    let expected = instruction("ADD R1, R2, R3");
    assert_eq!(instruction("ADD R1,R2,R3"), expected);
    assert_eq!(instruction("  \t ADD\tR1 ,  R2 , R3  ; комментарий"), expected);
    assert_eq!(instruction("LOAD R1,4( R2 )"), instruction("LOAD R1, 4(R2)"));
}

#[test]
fn register_like_names_are_symbols_if_not_registers() {
    // R16 не регистр, поэтому это имя
    assert_eq!(instruction("ADDI R1, R2, R16"), ("ADDI", vec![reg(1), reg(2), sym("R16")]));
    assert_eq!(instruction("JUMP R_END"), ("JUMP", vec![sym("R_END")]));
}

#[test]
fn labels() {
    let parsed = lines("loop:");
    assert_eq!(parsed, vec![Line { line: 1, labels: vec!["LOOP".into()], statement: None }]);

    let parsed = lines("loop: HALT");
    assert_eq!(parsed[0].labels, vec!["LOOP".to_string()]);
    assert!(matches!(parsed[0].statement, Some(Statement::Instruction { .. })));

    let parsed = lines("a: b: NOP");
    assert_eq!(parsed[0].labels, vec!["A".to_string(), "B".to_string()]);

    let parsed = lines("arr: .word 1, 2");
    assert_eq!(parsed[0].labels, vec!["ARR".to_string()]);
    assert!(matches!(parsed[0].statement, Some(Statement::Word(_))));

    let parsed = lines("start :\n  NOP");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].labels, vec!["START".to_string()]);
}

#[test]
fn label_may_have_the_name_of_a_command() {
    let parsed = lines("halt: NOP");
    assert_eq!(parsed[0].labels, vec!["HALT".to_string()]);
}

#[test]
fn line_numbers_are_kept_and_skipped_lines_are_counted() {
    let parsed = lines("; c\n\nloop:\n   NOP\n\n   HALT");
    let numbers: Vec<usize> = parsed.iter().map(|l| l.line).collect();
    assert_eq!(numbers, vec![3, 4, 6]);
}

#[test]
fn directives() {
    assert_eq!(statement(".text"), Statement::Text);
    assert_eq!(statement(".data"), Statement::Data);
    assert_eq!(statement(".equ led, -976"), Statement::Equ { name: "LED".into(), value: -976 });
    assert_eq!(statement(".EQU N, 0x10"), Statement::Equ { name: "N".into(), value: 16 });
    assert_eq!(statement(".org 16"), Statement::Org(Value::Number(16)));
    assert_eq!(statement(".org base"), Statement::Org(Value::Symbol("BASE".into())));
    assert_eq!(statement(".word 5"), Statement::Word(vec![Value::Number(5)]));
    assert_eq!(
        statement(".word 1, -2, 'a', ptr"),
        Statement::Word(vec![
            Value::Number(1),
            Value::Number(-2),
            Value::Number(97),
            Value::Symbol("PTR".into()),
        ])
    );
}

#[test]
fn several_lines() {
    let parsed = lines(".equ N, 3\n  .data\narr: .word 1\n  .text\n  LI R1, N\n  HALT\n");
    assert_eq!(parsed.len(), 6);
    assert_eq!(parsed[0].statement, Some(Statement::Equ { name: "N".into(), value: 3 }));
    assert_eq!(parsed[1].statement, Some(Statement::Data));
    assert_eq!(parsed[3].statement, Some(Statement::Text));
}

#[test]
fn instruction_errors() {
    let cases: [(&str, &str); 28] = [
        ("MUL R1, R2, R3", "неизвестная команда 'MUL'"),
        ("FETCH", "неизвестная команда 'FETCH'"),
        ("ADD R1, R2", "ADD ожидает операндов: 3, получено: 2"),
        ("ADD", "ADD ожидает операндов: 3, получено: 0"),
        ("ADD R1, R2, R3, R4", "ADD ожидает операндов: 3, получено: 4"),
        ("HALT R1", "HALT ожидает операндов: 0, получено: 1"),
        ("NOP R1", "NOP ожидает операндов: 0, получено: 1"),
        ("JR", "JR ожидает операндов: 1, получено: 0"),
        ("ADD R1, R2, 5", "операнд 3 команды ADD: ожидалось регистр"),
        ("ADD R1, 5, R3", "операнд 2 команды ADD: ожидалось регистр"),
        ("ADD loop, R2, R3", "операнд 1 команды ADD: ожидалось регистр"),
        ("ADDI R1, R2, R3", "операнд 3 команды ADDI: ожидалось число или имя"),
        ("LOAD R1, R2", "операнд 2 команды LOAD: ожидалось адрес вида imm(rs1)"),
        ("LOAD R1, 4", "операнд 2 команды LOAD: ожидалось адрес вида imm(rs1)"),
        ("ADD R1, R2, 4(R3)", "операнд 3 команды ADD: ожидалось регистр"),
        ("JUMP R1", "операнд 1 команды JUMP: ожидалось число или имя"),
        ("JUMP 4(R1)", "операнд 1 команды JUMP: ожидалось число или имя"),
        ("JR 5", "операнд 1 команды JR: ожидалось регистр"),
        ("JAL loop, R15", "операнд 1 команды JAL: ожидалось регистр"),
        ("ADD R1 R2, R3", "ожидалась ',' или конец строки"),
        ("ADD R1, R2, R3 R4", "ожидалась ',' или конец строки"),
        ("ADD R1,", "ожидался операнд"),
        ("ADD R1, , R3", "ожидался операнд"),
        ("ADD , R1", "ожидался операнд"),
        ("LOAD R1, 4(5)", "в скобках ожидался регистр"),
        ("LOAD R1, 4(R2", "ожидалась ')'"),
        ("LOAD R1, 4()", "в скобках ожидался регистр"),
        ("LOAD R1, (R2)", "ожидался операнд"),
    ];
    for (text, message) in cases {
        let e = error(text);
        assert_eq!(e.line, 1, "{text}");
        assert!(e.message.contains(message), "{text}: «{}», ждали «{message}»", e.message);
    }
}

#[test]
fn label_and_statement_start_errors() {
    let cases: [(&str, &str); 9] = [
        ("R1: NOP", "метка не может называться как регистр 'R1'"),
        ("R15:", "метка не может называться как регистр 'R15'"),
        (": NOP", "ожидалась команда или директива"),
        ("loop: : NOP", "ожидалась команда или директива"),
        ("5", "ожидалась команда или директива"),
        (", NOP", "ожидалась команда или директива"),
        ("(R1)", "ожидалась команда или директива"),
        ("R1", "неизвестная команда 'R1'"),
        ("loop: loop2", "неизвестная команда 'LOOP2'"),
    ];
    for (text, message) in cases {
        let e = error(text);
        assert_eq!(e.line, 1, "{text}");
        assert!(e.message.contains(message), "{text}: «{}», ждали «{message}»", e.message);
    }
}

#[test]
fn directive_errors() {
    let cases: [(&str, &str); 19] = [
        (".foo", "неизвестная директива '.foo'"),
        (".equ", ".equ: ожидалось имя константы"),
        (".equ X", ".equ: после имени ожидалась ','"),
        (".equ X,", ".equ: ожидалось значение"),
        (".equ X, Y", ".equ: значение должно быть числом"),
        (".equ R1, 5", ".equ: ожидалось имя константы (не регистр)"),
        (".equ 5, 5", ".equ: ожидалось имя константы (не регистр)"),
        (".equ X, 1, 2", "ожидался конец строки"),
        (".word", "ожидалось число или имя"),
        (".word 1,", "ожидалось число или имя"),
        (".word 1 2", "ожидался конец строки"),
        (".word R1", "ожидалось число или имя"),
        (".word (1)", "ожидалось число или имя"),
        (".org", "ожидалось число или имя"),
        (".org R1", "ожидалось число или имя"),
        (".org 1, 2", "ожидался конец строки"),
        (".text 5", "ожидался конец строки"),
        (".data x", "ожидался конец строки"),
        ("loop: .equ", ".equ: ожидалось имя константы"),
    ];
    for (text, message) in cases {
        let e = error(text);
        assert_eq!(e.line, 1, "{text}");
        assert!(e.message.contains(message), "{text}: «{}», ждали «{message}»", e.message);
    }
}

#[test]
fn error_position() {
    let e = error("NOP\n\n  ADD R1, R2");
    assert_eq!(e.line, 3);
    assert!(e.message.starts_with("столбец 3:"), "{}", e.message);

    let e = error("ADD R1, R2, 5");
    assert!(e.message.starts_with("столбец 13:"), "{}", e.message);

    // ошибка «не хватает операнда» указывает на конец строки
    let e = error("ADD R1,");
    assert!(e.message.starts_with("столбец 8:"), "{}", e.message);
}

#[test]
fn the_user_program_parses() {
    let text = std::fs::read_to_string("asm/duplicates_to_zero.asm").unwrap();
    let parsed = lines(&text);
    let labels: HashSet<String> = parsed.iter().flat_map(|l| l.labels.clone()).collect();
    for name in ["ARR", "RES", "START", "LOOP", "SKIP"] {
        assert!(labels.contains(name), "нет метки {name}");
    }
    let commands = parsed
        .iter()
        .filter(|l| matches!(l.statement, Some(Statement::Instruction { .. })))
        .count();
    assert!(commands > 5, "команд: {commands}");
}
