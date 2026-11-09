use soc::isa::{find, register, Op, Shape, INSTRUCTIONS};
use std::collections::HashSet;

/// Запись операндов в docs/ISA.md для каждой формы.
fn syntax(shape: Shape) -> &'static str {
    match shape {
        Shape::None => "—",
        Shape::RdRs1Rs2 => "rd, rs1, rs2",
        Shape::RdRs1Imm => "rd, rs1, imm",
        Shape::RdRs1 => "rd, rs1",
        Shape::Rs1Rs2 => "rs1, rs2",
        Shape::Rs1Imm => "rs1, imm",
        Shape::RdMem => "rd, imm(rs1)",
        Shape::Imm => "imm",
        Shape::RdImm => "rd, imm",
        Shape::Rs1 => "rs1",
        Shape::Rd => "rd",
    }
}

fn real_opcode(name: &str) -> u32 {
    match find(name).unwrap_or_else(|| panic!("нет команды {name}")).op {
        Op::Real(code) => code,
        other => panic!("{name}: не настоящая команда: {other:?}"),
    }
}

#[test]
fn table_matches_isa_md() {
    let doc = std::fs::read_to_string("docs/ISA.md").unwrap();

    // строки таблицы команд: | `0x01` | `ADD` | `rd, rs1, rs2` | ...
    let mut rows: Vec<(u32, String, String)> = Vec::new();
    for line in doc.lines().filter(|l| l.starts_with("| `0x")) {
        let cells: Vec<&str> = line.split('|').map(|c| c.trim().trim_matches('`')).collect();
        let code = u32::from_str_radix(cells[1].trim_start_matches("0x"), 16).unwrap();
        rows.push((code, cells[2].to_string(), cells[3].to_string()));
    }
    assert!(rows.len() > 30, "таблица команд в ISA.md не найдена: {} строк", rows.len());

    let mut seen = HashSet::new();
    for (code, name, operands) in &rows {
        if name == "FETCH" {
            continue; // служебный код, в программах не используется
        }
        let spec = find(name).unwrap_or_else(|| panic!("{name} есть в ISA.md, но нет в таблице"));
        assert_eq!(spec.op, Op::Real(*code), "{name}: код операции");
        assert_eq!(syntax(spec.shape), operands, "{name}: операнды");
        seen.insert(name.clone());
    }

    for spec in INSTRUCTIONS {
        if let Op::Real(_) = spec.op {
            assert!(seen.contains(spec.name), "{} есть в таблице, но нет в ISA.md", spec.name);
        }
    }
}

#[test]
fn names_and_opcodes_are_unique() {
    let mut names = HashSet::new();
    let mut codes = HashSet::new();
    for spec in INSTRUCTIONS {
        assert!(names.insert(spec.name), "имя {} повторяется", spec.name);
        if let Op::Real(code) = spec.op {
            assert!(code < 64, "{}: код не помещается в 6 бит", spec.name);
            assert_ne!(code, 0x3F, "{}: 0x3F занят под FETCH", spec.name);
            assert!(codes.insert(code), "код {code:#x} повторяется ({})", spec.name);
        }
    }
}

#[test]
fn names_are_upper_case_words() {
    for spec in INSTRUCTIONS {
        assert!(
            spec.name.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            "{}",
            spec.name
        );
    }
}

#[test]
fn immediate_form_is_register_form_plus_0x10() {
    for name in ["ADD", "SUB", "AND", "OR", "XOR", "SHL", "SHR", "SAR", "CMP"] {
        let immediate = format!("{name}I");
        assert_eq!(real_opcode(&immediate), real_opcode(name) + 0x10, "{immediate}");
    }
}

#[test]
fn find_by_name() {
    assert_eq!(find("ADD").unwrap().name, "ADD");
    assert_eq!(find("HIGH_RES_TICK").unwrap().shape, Shape::Rd);
    assert_eq!(find("LOAD").unwrap().shape, Shape::RdMem);
    assert_eq!(find("JAL").unwrap().shape, Shape::RdImm);
    assert!(find("add").is_none(), "имена ищутся в верхнем регистре");
    assert!(find("FETCH").is_none());
    assert!(find("").is_none());
    assert!(find("MUL").is_none());
    for part in ["AD", "HAL", "LOA", "J", "HIGH_RES", "ADDII", "ADD "] {
        assert!(find(part).is_none(), "{part:?}");
    }
}

#[test]
fn pseudo_instructions() {
    assert_eq!(find("NOP").unwrap().op, Op::Nop);
    assert_eq!(find("NOP").unwrap().shape, Shape::None);
    assert_eq!(find("MOV").unwrap().op, Op::Mov);
    assert_eq!(find("MOV").unwrap().shape, Shape::RdRs1);
    assert_eq!(find("LI").unwrap().op, Op::Li);
    assert_eq!(find("LI").unwrap().shape, Shape::RdImm);
}

#[test]
fn operand_counts_per_shape() {
    let cases = [
        (Shape::None, 0),
        (Shape::Rd, 1),
        (Shape::Rs1, 1),
        (Shape::Imm, 1),
        (Shape::RdRs1, 2),
        (Shape::Rs1Rs2, 2),
        (Shape::Rs1Imm, 2),
        (Shape::RdImm, 2),
        (Shape::RdMem, 2),
        (Shape::RdRs1Rs2, 3),
        (Shape::RdRs1Imm, 3),
    ];
    for (shape, count) in cases {
        assert_eq!(shape.operands().len(), count, "{shape:?}");
    }
}

#[test]
fn registers() {
    for number in 0..=15u8 {
        assert_eq!(register(&format!("R{number}")), Some(number), "R{number}");
    }
    for bad in ["R16", "R99", "R100", "R", "R01", "R00", "R-1", "RX", "R1X", "R 1", "", "r1", "X1", "1", "RR1", "R+1", "R٣"] {
        assert_eq!(register(bad), None, "{bad:?}");
    }
}
