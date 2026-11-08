use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use soc::ALU;

// Коды операций, см. docs/ALU.md
const ADD: u8 = 0b0000;
const SUB: u8 = 0b0001;
const AND: u8 = 0b0010;
const OR: u8 = 0b0011;
const XOR: u8 = 0b0100;
const NOT: u8 = 0b0101;
const SHL: u8 = 0b0110;
const SHR: u8 = 0b0111;
const SAR: u8 = 0b1000;

// Флаги nzcv = {N, Z, C, V}
const NONE: u8 = 0b0000;
const N: u8 = 0b1000;
const Z: u8 = 0b0100;
const C: u8 = 0b0010;
const V: u8 = 0b0001;

const MAX: u32 = u32::MAX; // 0xFFFF_FFFF, как знаковое это -1
const IMAX: u32 = 0x7FFF_FFFF; // наибольшее знаковое
const IMIN: u32 = 0x8000_0000; // наименьшее знаковое

struct Case {
    name: &'static str,
    a: u32,
    b: u32,
    op: u8,
    r: u32,
    nzcv: u8,
}

const fn case(name: &'static str, op: u8, a: u32, b: u32, r: u32, nzcv: u8) -> Case {
    Case {
        name,
        a,
        b,
        op,
        r,
        nzcv,
    }
}

const ADD_CASES: &[Case] = &[
    // обычные случаи
    case("add 1+1", ADD, 1, 1, 2, NONE),
    case("add x+0", ADD, 0x1234_5678, 0, 0x1234_5678, NONE),
    case("add 5+(-3)", ADD, 5, 0xFFFF_FFFD, 2, C),
    case("add -5+3", ADD, 0xFFFF_FFFB, 3, 0xFFFF_FFFE, N),
    case("add carry across 16 bits", ADD, 0xFFFF, 1, 0x1_0000, NONE),
    // ноль
    case("add 0+0", ADD, 0, 0, 0, Z),
    // перенос без знакового переполнения
    case("add max+1 wraps to zero", ADD, MAX, 1, 0, Z | C),
    case("add max+max", ADD, MAX, MAX, 0xFFFF_FFFE, N | C),
    case("add imax+max", ADD, IMAX, MAX, 0x7FFF_FFFE, C),
    // знаковое переполнение
    case("add imax+1", ADD, IMAX, 1, IMIN, N | V),
    case("add imax+imax", ADD, IMAX, IMAX, 0xFFFF_FFFE, N | V),
    case("add imin+imin", ADD, IMIN, IMIN, 0, Z | C | V),
    case("add imin+max", ADD, IMIN, MAX, IMAX, C | V),
    // числа разных знаков: переполнения быть не может
    case("add imax+imin", ADD, IMAX, IMIN, MAX, N),
];

const SUB_CASES: &[Case] = &[
    // обычные случаи
    case("sub 5-3", SUB, 5, 3, 2, NONE),
    case("sub 1-0", SUB, 1, 0, 1, NONE),
    // заём (a < b как беззнаковые)
    case("sub 3-5 borrow", SUB, 3, 5, 0xFFFF_FFFE, N | C),
    case("sub 0-1 borrow", SUB, 0, 1, MAX, N | C),
    case("sub 1-max borrow", SUB, 1, MAX, 2, C),
    case("sub imin-max", SUB, IMIN, MAX, 0x8000_0001, N | C),
    // ноль
    case("sub 0-0", SUB, 0, 0, 0, Z),
    case("sub 5-5", SUB, 5, 5, 0, Z),
    case("sub imin-imin", SUB, IMIN, IMIN, 0, Z),
    case("sub max-max", SUB, MAX, MAX, 0, Z),
    // числа одного знака: переполнения быть не может
    case("sub max-1", SUB, MAX, 1, 0xFFFF_FFFE, N),
    case("sub max-imin", SUB, MAX, IMIN, IMAX, NONE),
    // знаковое переполнение
    case("sub imin-1", SUB, IMIN, 1, IMAX, V),
    case("sub imin-imax", SUB, IMIN, IMAX, 1, V),
    case("sub imax-max (imax - (-1))", SUB, IMAX, MAX, IMIN, N | C | V),
    case("sub 0-imin", SUB, 0, IMIN, IMIN, N | C | V),
    case("sub imax-imin", SUB, IMAX, IMIN, MAX, N | C | V),
];

const LOGIC_CASES: &[Case] = &[
    // AND
    case("and mask", AND, 0xF0F0_F0F0, 0xFF00_FF00, 0xF000_F000, N),
    case("and to zero", AND, 0xAAAA_AAAA, 0x5555_5555, 0, Z),
    case("and with ones", AND, 0x1234_5678, MAX, 0x1234_5678, NONE),
    case("and max&max (C and V stay 0)", AND, MAX, MAX, MAX, N),
    // OR
    case("or full", OR, 0xF0F0_F0F0, 0x0F0F_0F0F, MAX, N),
    case("or 0|0", OR, 0, 0, 0, Z),
    case("or small", OR, 0xF0, 0x0F, 0xFF, NONE),
    // XOR
    case("xor same", XOR, MAX, MAX, 0, Z),
    case("xor full", XOR, 0xAAAA_AAAA, 0x5555_5555, MAX, N),
    case("xor x^0", XOR, 0x1234_5678, 0, 0x1234_5678, NONE),
    // NOT
    case("not 0", NOT, 0, 0, MAX, N),
    case("not max", NOT, MAX, 0, 0, Z),
    case("not 0xFFFF", NOT, 0xFFFF, 0, 0xFFFF_0000, N),
    case("not ignores b", NOT, 0, 0xDEAD_BEEF, MAX, N),
];

// Сдвиги: величина — b[4:0], C и V всегда 0
const SHL_CASES: &[Case] = &[
    case("shl 1<<0", SHL, 1, 0, 1, NONE),
    case("shl 1<<1", SHL, 1, 1, 2, NONE),
    case("shl 1<<31", SHL, 1, 31, IMIN, N),
    case("shl imin<<1 (bit shifted out)", SHL, IMIN, 1, 0, Z),
    case("shl max<<4", SHL, MAX, 4, 0xFFFF_FFF0, N),
    case("shl x<<8", SHL, 0x1234_5678, 8, 0x3456_7800, NONE),
    case("shl 0<<5", SHL, 0, 5, 0, Z),
    case("shl ignores b[31:5]", SHL, 1, 0x21, 2, NONE),
    case("shl by 32 is shift by 0", SHL, 0x1234_5678, 32, 0x1234_5678, NONE),
];

const SHR_CASES: &[Case] = &[
    case("shr imin>>31", SHR, IMIN, 31, 1, NONE),
    case("shr imin>>1 (no sign fill)", SHR, IMIN, 1, 0x4000_0000, NONE),
    case("shr max>>4", SHR, MAX, 4, 0x0FFF_FFFF, NONE),
    case("shr max>>31", SHR, MAX, 31, 1, NONE),
    case("shr 1>>1", SHR, 1, 1, 0, Z),
    case("shr x>>0", SHR, 0xF000_0000, 0, 0xF000_0000, N),
    case("shr x>>8", SHR, 0x1234_5678, 8, 0x0012_3456, NONE),
];

const SAR_CASES: &[Case] = &[
    case("sar imin>>31", SAR, IMIN, 31, MAX, N),
    case("sar imin>>1 (sign fill)", SAR, IMIN, 1, 0xC000_0000, N),
    case("sar max>>4", SAR, MAX, 4, MAX, N),
    case("sar positive>>30", SAR, 0x4000_0000, 30, 1, NONE),
    case("sar imax>>31", SAR, IMAX, 31, 0, Z),
    case("sar 0xFFFFFFF0>>4", SAR, 0xFFFF_FFF0, 4, MAX, N),
    case("sar negative>>0", SAR, 0x8000_0001, 0, 0x8000_0001, N),
    case("sar 0xF0000000>>28", SAR, 0xF000_0000, 28, MAX, N),
];

// Неиспользуемые коды: r = 0, значит Z = 1
const UNUSED_CASES: &[Case] = &[
    case("op 1001", 0b1001, 0x1234_5678, 0x8765_4321, 0, Z),
    case("op 1111", 0b1111, 0x1234_5678, 0x8765_4321, 0, Z),
];

fn run(cases: &[Case]) -> Result<(), VerilatorError> {
    let runtime = VerilatorRuntime::new(
        "build".as_ref(),
        &["rtl/alu.v".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )?;

    let mut alu = runtime.create_model_simple::<ALU>()?;

    let mut failures = Vec::new();
    for c in cases {
        alu.a = c.a;
        alu.b = c.b;
        alu.op = c.op;

        alu.eval();

        if alu.r != c.r || alu.nzcv != c.nzcv {
            failures.push(format!(
                "{}: op={:04b} a={:#010x} b={:#010x}: ожидалось r={:#010x} nzcv={:04b}, получено r={:#010x} nzcv={:04b}",
                c.name, c.op, c.a, c.b, c.r, c.nzcv, alu.r, alu.nzcv
            ));
        }
    }

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));

    Ok(())
}

#[test]
#[snafu::report]
fn alu_add() -> Result<(), VerilatorError> {
    run(ADD_CASES)
}

#[test]
#[snafu::report]
fn alu_sub() -> Result<(), VerilatorError> {
    run(SUB_CASES)
}

#[test]
#[snafu::report]
fn alu_logic() -> Result<(), VerilatorError> {
    run(LOGIC_CASES)
}

#[test]
#[snafu::report]
fn alu_shl() -> Result<(), VerilatorError> {
    run(SHL_CASES)
}

#[test]
#[snafu::report]
fn alu_shr() -> Result<(), VerilatorError> {
    run(SHR_CASES)
}

#[test]
#[snafu::report]
fn alu_sar() -> Result<(), VerilatorError> {
    run(SAR_CASES)
}

#[test]
#[snafu::report]
fn alu_unused_op() -> Result<(), VerilatorError> {
    run(UNUSED_CASES)
}
