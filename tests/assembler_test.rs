use soc::assemble;
use soc::image::{Image, Memory};
use soc::isa::opcode::*;

// Ожидаемые слова считаются по формату из docs/ISA.md, независимо от кода ассемблера
fn rf(op: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (op << 26) | (rd << 22) | (rs1 << 18) | (rs2 << 14)
}

fn im(op: u32, rd: u32, rs1: u32, imm: i32) -> u32 {
    (op << 26) | (rd << 22) | (rs1 << 18) | ((imm as u32) & 0x3_FFFF)
}

fn asm(text: &str) -> Image {
    assemble(text).unwrap_or_else(|e| panic!("{text:?}: {e}"))
}

fn error(text: &str) -> soc::error::Error {
    assemble(text).expect_err(text)
}

/// Слова образа подряд с адреса 0 до первого пропуска.
fn words(memory: &Memory, count: u32) -> Vec<u32> {
    (0..count)
        .map(|a| memory.get(a).unwrap_or_else(|| panic!("нет слова по адресу {a}")))
        .collect()
}

fn rom(text: &str) -> Vec<u32> {
    let image = asm(text);
    words(&image.rom, image.rom.len() as u32)
}

fn one(text: &str) -> u32 {
    let w = rom(text);
    assert_eq!(w.len(), 1, "{text}");
    w[0]
}

#[test]
fn empty_program_gives_empty_images() {
    let image = asm("");
    assert!(image.rom.is_empty() && image.ram.is_empty());
    assert_eq!(image.rom.to_hex(), "");
    let image = asm("; только комментарий\n\n");
    assert_eq!(image, Image::default());
}

#[test]
fn every_instruction_is_encoded_by_its_format() {
    let cases: Vec<(&str, u32)> = vec![
        ("HALT", 0),
        ("ADD R1, R2, R3", rf(ADD, 1, 2, 3)),
        ("SUB R15, R14, R13", rf(SUB, 15, 14, 13)),
        ("AND R1, R2, R3", rf(AND, 1, 2, 3)),
        ("OR R1, R2, R3", rf(OR, 1, 2, 3)),
        ("XOR R4, R5, R6", rf(XOR, 4, 5, 6)),
        ("NOT R1, R2", rf(NOT, 1, 2, 0)),
        ("SHL R1, R2, R3", rf(SHL, 1, 2, 3)),
        ("SHR R1, R2, R3", rf(SHR, 1, 2, 3)),
        ("SAR R1, R2, R3", rf(SAR, 1, 2, 3)),
        ("CMP R3, R4", rf(CMP, 0, 3, 4)),
        ("ADDI R1, R2, 5", im(ADDI, 1, 2, 5)),
        ("SUBI R1, R2, 5", im(SUBI, 1, 2, 5)),
        ("ANDI R1, R2, 255", im(ANDI, 1, 2, 255)),
        ("ORI R1, R2, 0x10", im(ORI, 1, 2, 16)),
        ("XORI R1, R2, 0b101", im(XORI, 1, 2, 5)),
        ("SHLI R1, R2, 3", im(SHLI, 1, 2, 3)),
        ("SHRI R1, R2, 3", im(SHRI, 1, 2, 3)),
        ("SARI R1, R2, 3", im(SARI, 1, 2, 3)),
        ("CMPI R5, 9", im(CMPI, 0, 5, 9)),
        ("LOAD R1, 4(R2)", im(LOAD, 1, 2, 4)),
        ("STORE R1, 4(R2)", im(STORE, 1, 2, 4)),
        ("STORE R1, -976(R0)", im(STORE, 1, 0, -976)),
        ("JUMP -3", im(JUMP, 0, 0, -3)),
        ("BEQ 2", im(BEQ, 0, 0, 2)),
        ("BNE 2", im(BNE, 0, 0, 2)),
        ("BLTU 2", im(BLTU, 0, 0, 2)),
        ("BGEU 2", im(BGEU, 0, 0, 2)),
        ("JAL R15, 7", im(JAL, 15, 0, 7)),
        ("JR R15", rf(JR, 0, 15, 0)),
        ("HIGH_RES_TICK R7", rf(HIGH_RES_TICK, 7, 0, 0)),
    ];
    for (text, expected) in cases {
        assert_eq!(one(text), expected, "{text}: {:#010x} ≠ {expected:#010x}", one(text));
    }
}

#[test]
fn pseudo_instructions_expand_to_real_ones() {
    assert_eq!(one("NOP"), rf(ADD, 0, 0, 0));
    assert_eq!(one("MOV R1, R2"), rf(ADD, 1, 2, 0));
    assert_eq!(one("MOV R15, R0"), rf(ADD, 15, 0, 0));
    assert_eq!(one("LI R1, 100"), im(ADDI, 1, 0, 100));
    assert_eq!(one("LI R1, -1"), im(ADDI, 1, 0, -1));
    assert_eq!(one("LI R3, 0x1FFFF"), im(ADDI, 3, 0, 0x1FFFF));
}

#[test]
fn case_does_not_matter() {
    assert_eq!(rom("add r1, r2, r3"), rom("ADD R1, R2, R3"));
    assert_eq!(rom("Loop: bne LOOP"), rom("LOOP: BNE loop"));
}

#[test]
fn immediate_range_is_18_bits_signed() {
    assert_eq!(one("ADDI R1, R2, 131071"), im(ADDI, 1, 2, 131071));
    assert_eq!(one("ADDI R1, R2, -131072"), im(ADDI, 1, 2, -131072));
    assert_eq!(one("ADDI R1, R2, -131072") & 0x3_FFFF, 0x2_0000);
    for text in ["ADDI R1, R2, 131072", "ADDI R1, R2, -131073", "LI R1, 0x20000", "LOAD R1, 131072(R0)", "CMPI R1, 99999999"] {
        let e = error(text);
        assert!(e.message.contains("не помещается в 18 бит"), "{text}: {}", e.message);
        assert_eq!(e.line, 1);
    }
}

#[test]
fn backward_and_forward_branches_are_relative_to_the_next_instruction() {
    let w = rom(
        "loop:    NOP              ; 0
                 BNE   loop       ; 1: 0 - 2 = -2
                 BEQ   fwd        ; 2: 4 - 3 = 1
                 NOP              ; 3
         fwd:    JUMP  fwd        ; 4: 4 - 5 = -1
                 JAL   R15, loop  ; 5: 0 - 6 = -6
                 BLTU  fwd        ; 6: 4 - 7 = -3
                 BGEU  end        ; 7: 9 - 8 = 1
                 NOP              ; 8
         end:    HALT             ; 9",
    );
    assert_eq!(
        w,
        vec![
            rf(ADD, 0, 0, 0),
            im(BNE, 0, 0, -2),
            im(BEQ, 0, 0, 1),
            rf(ADD, 0, 0, 0),
            im(JUMP, 0, 0, -1),
            im(JAL, 15, 0, -6),
            im(BLTU, 0, 0, -3),
            im(BGEU, 0, 0, 1),
            rf(ADD, 0, 0, 0),
            0,
        ]
    );
}

#[test]
fn branch_to_the_next_instruction_has_zero_offset() {
    assert_eq!(rom("JUMP next\nnext: HALT")[0], im(JUMP, 0, 0, 0));
}

#[test]
fn branch_with_a_number_or_a_constant_uses_it_as_the_offset() {
    assert_eq!(one("BEQ -3"), im(BEQ, 0, 0, -3));
    assert_eq!(one(".equ off, 5\nBEQ off").to_owned(), im(BEQ, 0, 0, 5));
}

#[test]
fn branch_to_a_data_label_is_an_error() {
    let e = error(".data\nd: .word 1\n.text\nJUMP d");
    assert_eq!(e.line, 4);
    assert!(e.message.contains("лежит в .data"), "{}", e.message);
}

#[test]
fn labels_in_text_are_rom_addresses_and_in_data_are_ram_addresses() {
    let image = asm(
        ".data
         a:  .word 10, 20
         b:  .word 30
         .text
         t0: NOP
         t1: LI R1, a
             LI R2, b
             LI R3, t1
             LOAD R4, b(R0)
             LOAD R5, a(R3)",
    );
    assert_eq!(
        words(&image.rom, 6),
        vec![
            rf(ADD, 0, 0, 0),
            im(ADDI, 1, 0, 0), // a = 0
            im(ADDI, 2, 0, 2), // b = 2
            im(ADDI, 3, 0, 1), // t1 = 1 (ROM)
            im(LOAD, 4, 0, 2),
            im(LOAD, 5, 3, 0),
        ]
    );
    assert_eq!(words(&image.ram, 3), vec![10, 20, 30]);
}

#[test]
fn names_may_be_used_before_they_are_defined() {
    let w = rom("LI R1, later\nLOAD R2, arr(R0)\n.equ later, 7\n.data\n.org 9\narr: .word 1");
    assert_eq!(w, vec![im(ADDI, 1, 0, 7), im(LOAD, 2, 0, 9)]);
}

#[test]
fn equ_constants() {
    let w = rom(".equ LED, -976\n.equ n, 0x10\nSTORE R1, LED(R0)\nADDI R2, R2, n");
    assert_eq!(w, vec![im(STORE, 1, 0, -976), im(ADDI, 2, 2, 16)]);
}

#[test]
fn word_directive() {
    let image = asm(".data\n.word 1, -1, 0xFFFFFFFF, 'a', 0x80000000, -2147483648\n.word 7");
    assert_eq!(
        words(&image.ram, 7),
        vec![1, 0xFFFF_FFFF, 0xFFFF_FFFF, 97, 0x8000_0000, 0x8000_0000, 7]
    );
    assert!(image.rom.is_empty());
}

#[test]
fn word_may_hold_a_name() {
    let image = asm(".equ k, 5\n.data\nstart: .word k, here, start\nhere: .word 0");
    assert_eq!(words(&image.ram, 4), vec![5, 3, 0, 0]);
}

#[test]
fn word_range_is_32_bits() {
    asm(".data\n.word 4294967295");
    asm(".data\n.word -2147483648");
    for text in [".data\n.word 4294967296", ".data\n.word -2147483649", ".data\n.word 1, 0x1FFFFFFFF"] {
        let e = error(text);
        assert!(e.message.contains("не помещается в 32 бита"), "{text}: {}", e.message);
        assert_eq!(e.line, 2);
    }
}

#[test]
fn text_and_data_counters_are_independent_and_sections_may_alternate() {
    let image = asm(
        "NOP
         .data
         .word 1
         .text
         HALT
         .data
         .word 2, 3
         .text
         NOP",
    );
    assert_eq!(words(&image.rom, 3), vec![rf(ADD, 0, 0, 0), 0, rf(ADD, 0, 0, 0)]);
    assert_eq!(words(&image.ram, 3), vec![1, 2, 3]);
}

#[test]
fn code_starts_in_text_without_any_directive() {
    assert_eq!(rom("HALT").len(), 1);
}

#[test]
fn org_sets_the_address_of_the_current_section() {
    let image = asm(".org 4\nNOP\nHALT\n.data\n.org 16\n.word 5, 6");
    assert_eq!(image.rom.get(4), Some(rf(ADD, 0, 0, 0)));
    assert_eq!(image.rom.get(5), Some(0));
    assert_eq!(image.rom.len(), 2);
    assert_eq!(image.ram.get(16), Some(5));
    assert_eq!(image.ram.get(17), Some(6));
    assert_eq!(image.ram.len(), 2);
}

#[test]
fn org_may_go_backwards_and_the_later_write_wins() {
    let image = asm(".data\n.word 1, 2, 3, 4, 5\n.org 2\n.word 99");
    assert_eq!(words(&image.ram, 5), vec![1, 2, 99, 4, 5]);
    assert_eq!(image.ram.len(), 5);

    let image = asm(".org 3\nHALT\n.org 0\nNOP\nNOP\nNOP\nBEQ 1\n.org 3\nJUMP 0");
    assert_eq!(image.rom.get(3), Some(im(JUMP, 0, 0, 0)));
    assert_eq!(image.rom.len(), 4);
}

#[test]
fn org_may_go_forward_and_jump_around() {
    let image = asm(".data\n.org 10\n.word 1\n.org 2\n.word 2\n.org 1023\n.word 3");
    assert_eq!(image.ram.get(10), Some(1));
    assert_eq!(image.ram.get(2), Some(2));
    assert_eq!(image.ram.get(1023), Some(3));
    assert_eq!(image.ram.len(), 3);
}

#[test]
fn org_sections_do_not_affect_each_other() {
    let image = asm(".org 8\n.data\n.org 2\n.word 1\n.text\nNOP");
    assert_eq!(image.rom.get(8), Some(rf(ADD, 0, 0, 0)));
    assert_eq!(image.ram.get(2), Some(1));
}

#[test]
fn org_accepts_constants_and_earlier_labels() {
    let image = asm(".equ base, 20\n.data\n.org base\n.word 1\nhere: .org here\n.word 2");
    // here = 21, после .org here счётчик снова 21
    assert_eq!(image.ram.get(20), Some(1));
    assert_eq!(image.ram.get(21), Some(2));
}

#[test]
fn a_label_takes_the_address_before_the_org_on_the_same_line() {
    let image = asm(".data\nbefore: .org 16\nafter:\n.text\nLI R1, before\nLI R2, after");
    assert_eq!(words(&image.rom, 2), vec![im(ADDI, 1, 0, 0), im(ADDI, 2, 0, 16)]);
}

#[test]
fn org_errors() {
    let cases: [(&str, &str); 5] = [
        (".org -1", "вне образа"),
        (".org 1024", "вне образа"),
        (".org later\n.equ later, 5", "должно быть определено выше"),
        (".org nowhere", "должно быть определено выше"),
        (".data\n.org 99999", "вне образа"),
    ];
    for (text, message) in cases {
        let e = error(text);
        assert!(e.message.contains(message), "{text}: {}", e.message);
    }
    asm(".org 0");
    asm(".org 1023");
}

#[test]
fn image_size_is_1024_words() {
    let full = "NOP\n".repeat(1024);
    assert_eq!(asm(&full).rom.len(), 1024);

    let too_long = "NOP\n".repeat(1025);
    let e = error(&too_long);
    assert_eq!(e.line, 1025);
    assert!(e.message.contains("выходит за размер образа"), "{}", e.message);

    let e = error(".data\n.org 1023\n.word 1, 2");
    assert_eq!(e.line, 3);
    assert!(e.message.contains("выходит за размер образа"), "{}", e.message);
}

#[test]
fn name_errors() {
    let cases: [(&str, usize, &str); 8] = [
        ("a: NOP\na: NOP", 2, "имя 'A' уже определено"),
        ("a: NOP\n.equ a, 1", 2, "имя 'A' уже определено"),
        (".equ a, 1\n.equ a, 2", 2, "имя 'A' уже определено"),
        (".equ a, 1\na:", 2, "имя 'A' уже определено"),
        ("a: b: a: NOP", 1, "имя 'A' уже определено"),
        ("JUMP nowhere", 1, "неизвестное имя 'NOWHERE'"),
        ("NOP\nLI R1, ghost", 2, "неизвестное имя 'GHOST'"),
        (".data\n.word ghost", 2, "неизвестное имя 'GHOST'"),
    ];
    for (text, line, message) in cases {
        let e = error(text);
        assert_eq!(e.line, line, "{text}");
        assert!(e.message.contains(message), "{text}: {}", e.message);
    }
}

#[test]
fn section_errors() {
    let e = error(".word 1");
    assert!(e.message.contains(".word допустим только в .data"), "{}", e.message);
    let e = error(".data\nNOP");
    assert_eq!(e.line, 2);
    assert!(e.message.contains("допустима только в .text"), "{}", e.message);
    let e = error(".data\nADD R1, R2, R3");
    assert!(e.message.contains("ADD"), "{}", e.message);
    // после .text снова можно писать команды
    asm(".data\n.word 1\n.text\nNOP");
}

#[test]
fn parser_and_lexer_errors_pass_through() {
    assert!(error("ADD R1, R2").message.contains("ожидает операндов"));
    assert!(error("NOP @").message.contains("неожиданный символ"));
    assert_eq!(error("NOP\nNOP\nFOO").line, 3);
}

#[test]
fn hex_format() {
    let mut memory = Memory::default();
    assert_eq!(memory.to_hex(), "");
    memory.set(0, 0xDEADBEEF);
    memory.set(1, 0x1);
    memory.set(5, 0xA0000001);
    memory.set(0x3FF, 0xFFFF_FFFF);
    assert_eq!(
        memory.to_hex(),
        "@000\nDEADBEEF\n00000001\n@005\nA0000001\n@3FF\nFFFFFFFF\n"
    );
    memory.set(5, 2); // повторная запись заменяет слово
    assert_eq!(memory.get(5), Some(2));
    assert_eq!(memory.len(), 4);
    assert_eq!(memory.get(2), None);
}

#[test]
fn hex_of_a_program_starts_at_zero() {
    let image = asm("ADDI R1, R0, 5\nHALT");
    assert_eq!(image.rom.to_hex(), format!("@000\n{:08X}\n00000000\n", im(ADDI, 1, 0, 5)));
}

// ---------- программа из задания «3» ----------

const DUPLICATES: &str = "
.equ    arr_size, 10
.equ    last_el, 9

        .data
arr:    .word 1, 1, 2, 2, 2, 5, 5, 7, 8, 9
        .org 16
res:
        .text
start:
    MOV R1, R0
    LOAD R2, arr(R1)
    STORE R2, res(R0)

loop:
    ADDI R1, R1, 1
    LOAD R3, arr(R1)

    CMP R2, R3
    BEQ skip

    STORE R3, res(R1)

skip:
    MOV R2, R3
    CMPI R1, last_el
    BNE loop
    HALT
";

#[test]
fn duplicates_program_words() {
    let image = asm(DUPLICATES);
    assert_eq!(
        words(&image.rom, 12),
        vec![
            rf(ADD, 1, 0, 0),    // 0  MOV R1, R0
            im(LOAD, 2, 1, 0),   // 1  LOAD R2, arr(R1), arr = 0
            im(STORE, 2, 0, 16), // 2  STORE R2, res(R0), res = 16
            im(ADDI, 1, 1, 1),   // 3  loop
            im(LOAD, 3, 1, 0),   // 4
            rf(CMP, 0, 2, 3),    // 5
            im(BEQ, 0, 0, 1),    // 6  skip = 8: 8 - 7
            im(STORE, 3, 1, 16), // 7
            rf(ADD, 2, 3, 0),    // 8  skip: MOV R2, R3
            im(CMPI, 0, 1, 9),   // 9
            im(BNE, 0, 0, -8),   // 10 loop = 3: 3 - 11
            0,                   // 11 HALT
        ]
    );
    assert_eq!(image.rom.len(), 12);
    assert_eq!(words(&image.ram, 10), vec![1, 1, 2, 2, 2, 5, 5, 7, 8, 9]);
    assert_eq!(image.ram.len(), 10);
}
