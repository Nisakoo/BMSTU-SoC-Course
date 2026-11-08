use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use std::path::Path;
use std::sync::Mutex;

// soc_top с программой из build/soc_program.hex: tests/rtl/soc_test_top.v
#[verilog(src = "tests/rtl/soc_test_top.v", name = "soc_test_top")]
struct SocTestTop;

const SOC_FILES: [&str; 19] = [
    "rtl/main.v",
    "rtl/reg_we.v",
    "rtl/counter.v",
    "rtl/micro_rom.v",
    "rtl/rom.v",
    "rtl/regfile.v",
    "rtl/alu.v",
    "rtl/pc_logic.v",
    "rtl/addr_decoder.v",
    "rtl/ram.v",
    "rtl/vram.v",
    "rtl/dev_timer.v",
    "rtl/dev_capture.v",
    "rtl/dev_button.v",
    "rtl/dev_led.v",
    "rtl/dev_gate.v",
    "rtl/dev_display.v",
    "rtl/dev_ram_view.v",
    "tests/rtl/soc_test_top.v",
];

// Тесты пишут программу в один и тот же файл, а ROM читает его при первом eval():
// файл не должен меняться, пока модель не создана.
static PROGRAM_LOCK: Mutex<()> = Mutex::new(());

// ---------- коды операций (docs/ISA.md, раздел «Команды») ----------
const ADD: u32 = 0x01;
const SUB: u32 = 0x02;
const OR: u32 = 0x04;
const NOT: u32 = 0x06;
const CMP: u32 = 0x0A;
const ADDI: u32 = 0x11;
const SUBI: u32 = 0x12;
const ANDI: u32 = 0x13;
const ORI: u32 = 0x14;
const XORI: u32 = 0x15;
const SHLI: u32 = 0x17;
const SHRI: u32 = 0x18;
const SARI: u32 = 0x19;
const LOAD: u32 = 0x20;
const STORE: u32 = 0x21;
const JUMP: u32 = 0x30;
const BEQ: u32 = 0x31;
const BNE: u32 = 0x32;
const BLTU: u32 = 0x33;
const BGEU: u32 = 0x34;
const JAL: u32 = 0x35;
const JR: u32 = 0x36;
const HRT: u32 = 0x38; // HIGH_RES_TICK
const HALT: u32 = 0x00;

// Формат команды: opcode[31:26], rd[25:22], rs1[21:18], rs2[17:14] или imm[17:0]
const fn reg_form(op: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (op << 26) | (rd << 22) | (rs1 << 18) | (rs2 << 14)
}

const fn imm_form(op: u32, rd: u32, rs1: u32, imm: i32) -> u32 {
    (op << 26) | (rd << 22) | (rs1 << 18) | ((imm as u32) & 0x3_FFFF)
}

// Адрес MMIO через R0: смещение устройства минус 1024
const fn mmio(offset: i32) -> i32 {
    offset - 1024
}

const CAPTURE_ARM: i32 = mmio(0x010);
const REMOTE: i32 = mmio(0x020);
const LIMIT: i32 = mmio(0x021);
const IR_SENSOR: i32 = mmio(0x022);
const LED: i32 = mmio(0x030);
const GATE: i32 = mmio(0x040);

// Короткие записи команд
const fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    imm_form(ADDI, rd, rs1, imm)
}
const fn load(rd: u32, imm: i32) -> u32 {
    imm_form(LOAD, rd, 0, imm)
}
const fn store(rd: u32, imm: i32) -> u32 {
    imm_form(STORE, rd, 0, imm)
}
const fn branch(op: u32, imm: i32) -> u32 {
    imm_form(op, 0, 0, imm)
}

// Записывает программу в hex-файл для $readmemh; остальные слова ROM останутся нулями (HALT)
fn write_program(words: &[u32]) {
    std::fs::create_dir_all("build").expect("каталог build");
    let text: String = words.iter().map(|w| format!("{w:08X}\n")).collect();
    std::fs::write("build/soc_program.hex", text).expect("файл программы");
}

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

// Создаёт модель с программой и снимает сброс; на выходе вход сброса равен 0
macro_rules! start_soc {
    ($runtime:ident, $m:ident, $program:expr) => {
        let $runtime = {
            let files: Vec<&Path> = SOC_FILES.iter().map(|f| Path::new(*f)).collect();
            VerilatorRuntime::new(
                "build".as_ref(),
                &files,
                &[],
                [],
                VerilatorRuntimeOptions::default(),
            )?
        };
        let mut $m = {
            let _guard = PROGRAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            write_program($program);
            let mut m = $runtime.create_model_simple::<SocTestTop>()?;
            m.clk = 0;
            m.reset = 1;
            m.btn_react = 0;
            m.remote = 0;
            m.limit = 0;
            m.ir_sensor = 0;
            m.frame_addr = 0;
            m.ram_view_addr = 0;
            m.eval(); // здесь ROM читает файл программы
            m
        };
        tick!($m);
        tick!($m);
        $m.reset = 0;
        $m.eval();
    };
}

// Выполняет такты, пока выход halt не станет 1; возвращает число тактов
macro_rules! run_until_halt {
    ($m:expr, $limit:expr) => {{
        let mut ticks = 0u32;
        while $m.halt == 0 {
            assert!(ticks < $limit, "программа не остановилась за {} тактов", $limit);
            tick!($m);
            ticks += 1;
        }
        ticks
    }};
}

#[test]
#[snafu::report]
fn empty_rom_halts_after_the_first_fetch() -> Result<(), VerilatorError> {
    start_soc!(runtime, m, &[]);
    assert_eq!(m.halt, 0, "до первого такта идёт выборка");
    tick!(m);
    assert_eq!(m.halt, 1, "в ROM нули, то есть HALT");
    Ok(())
}

#[test]
#[snafu::report]
fn unknown_opcode_halts() -> Result<(), VerilatorError> {
    start_soc!(runtime, m, &[0x3E << 26]);
    let ticks = run_until_halt!(m, 10);
    assert_eq!(ticks, 1);
    assert_eq!(m.led, 0);
    Ok(())
}

#[test]
#[snafu::report]
fn arithmetic_result_is_shown_on_leds() -> Result<(), VerilatorError> {
    // r1 = 5, r2 = 7, r3 = r1 + r2, LED = r3
    let program = [
        addi(1, 0, 5),
        addi(2, 0, 7),
        reg_form(ADD, 3, 1, 2),
        store(3, LED),
        HALT,
    ];
    start_soc!(runtime, m, &program);
    let ticks = run_until_halt!(m, 100);
    assert_eq!(m.led, 12);
    // выборка и шаги: ADDI 2, ADDI 2, ADD 2, STORE 3, выборка HALT 1
    assert_eq!(ticks, 10, "число тактов");
    Ok(())
}

#[test]
#[snafu::report]
fn reset_restarts_the_program() -> Result<(), VerilatorError> {
    let program = [addi(1, 0, 12), store(1, LED), HALT];
    start_soc!(runtime, m, &program);
    run_until_halt!(m, 100);
    assert_eq!(m.led, 12);

    m.reset = 1;
    tick!(m);
    m.reset = 0;
    m.eval();
    assert_eq!(m.led, 0, "сброс выключил светодиоды");
    assert_eq!(m.halt, 0, "после сброса снова выборка");

    let ticks = run_until_halt!(m, 100);
    assert_eq!(m.led, 12, "программа выполнилась заново");
    assert_eq!(ticks, 2 + 3 + 1);
    Ok(())
}

#[test]
#[snafu::report]
fn loop_and_memory_work_together() -> Result<(), VerilatorError> {
    // сумма 5+4+3+2+1 в цикле, запись в RAM, чтение обратно, вывод на светодиоды
    let program = [
        addi(1, 0, 5),                  // 0: счётчик
        addi(2, 0, 0),                  // 1: сумма
        reg_form(ADD, 2, 2, 1),         // 2: loop: r2 += r1
        imm_form(SUBI, 1, 1, 1),        // 3: r1 -= 1, флаги обновляются
        branch(BNE, -3),                // 4: пока r1 != 0, на метку loop (адрес 2)
        store(2, 0),                    // 5: RAM[0] = r2
        load(3, 0),                     // 6: r3 = RAM[0]
        store(3, LED),                  // 7: LED = r3
        HALT,                           // 8
    ];
    start_soc!(runtime, m, &program);
    run_until_halt!(m, 500);
    assert_eq!(m.led, 15);
    Ok(())
}

#[test]
#[snafu::report]
fn logic_and_shifts() -> Result<(), VerilatorError> {
    // (программа, ожидаемое значение светодиодов)
    let cases: [(Vec<u32>, u8); 4] = [
        // 1 << 4 | 3 = 19
        (
            vec![addi(1, 0, 1), imm_form(SHLI, 1, 1, 4), imm_form(ORI, 1, 1, 3), store(1, LED), HALT],
            19,
        ),
        // (-16) >> 2 арифметически = -4, младший байт 0xFC
        (
            vec![addi(1, 0, -16), imm_form(SARI, 1, 1, 2), store(1, LED), HALT],
            0xFC,
        ),
        // (-16) >> 28 логически = 0xF
        (
            vec![addi(1, 0, -16), imm_form(SHRI, 1, 1, 28), store(1, LED), HALT],
            0x0F,
        ),
        // ~0x0F & 0xFF ^ 0xFF = 0x0F
        (
            vec![
                addi(2, 0, 0x0F),
                reg_form(NOT, 3, 2, 0),
                imm_form(ANDI, 3, 3, 0xFF),
                imm_form(XORI, 3, 3, 0xFF),
                store(3, LED),
                HALT,
            ],
            0x0F,
        ),
    ];
    for (i, (program, expected)) in cases.iter().enumerate() {
        start_soc!(runtime, m, program);
        run_until_halt!(m, 100);
        assert_eq!(m.led, *expected, "программа №{i}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn subtraction_and_register_forms() -> Result<(), VerilatorError> {
    // r1 = 100, r2 = 58, r3 = r1 - r2 = 42, r4 = r3 | r2 = 42 | 58 = 58, LED = r3 + 1
    let program = [
        addi(1, 0, 100),
        addi(2, 0, 58),
        reg_form(SUB, 3, 1, 2),
        reg_form(OR, 4, 3, 2),
        addi(5, 3, 1),
        store(5, LED),
        store(4, mmio(0x031)), // другой адрес MMIO, никого не затрагивает
        HALT,
    ];
    start_soc!(runtime, m, &program);
    run_until_halt!(m, 100);
    assert_eq!(m.led, 43);
    Ok(())
}

#[test]
#[snafu::report]
fn branches_use_the_right_flags() -> Result<(), VerilatorError> {
    // CMP a, b, затем переход вперёд на 2 команды; светодиоды: 1 — переход был, 2 — не было
    let cases: [(u32, i32, i32, u8); 11] = [
        (BEQ, 5, 5, 1),
        (BEQ, 5, 6, 2),
        (BNE, 5, 6, 1),
        (BNE, 5, 5, 2),
        (BLTU, 3, 5, 1),
        (BLTU, 5, 3, 2),
        (BLTU, 5, 5, 2),
        (BLTU, 1, -1, 1), // -1 = 0xFFFFFFFF как беззнаковое
        (BGEU, 5, 3, 1),
        (BGEU, 5, 5, 1),
        (BGEU, 3, 5, 2),
    ];
    for (op, a, b, expected) in cases {
        let program = [
            addi(1, 0, a),                  // 0
            addi(2, 0, b),                  // 1
            reg_form(CMP, 0, 1, 2),         // 2: флаги по a - b
            branch(op, 2),                  // 3: при условии на команду 6
            addi(3, 0, 2),                  // 4: перехода не было
            branch(JUMP, 1),                // 5: к команде 7
            addi(3, 0, 1),                  // 6: переход был
            store(3, LED),                  // 7
            HALT,                           // 8
        ];
        start_soc!(runtime, m, &program);
        run_until_halt!(m, 100);
        assert_eq!(m.led, expected, "переход {op:#04x}, a = {a}, b = {b}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn call_and_return_with_jal_and_jr() -> Result<(), VerilatorError> {
    let program = [
        addi(1, 0, 1),                  // 0
        imm_form(JAL, 15, 0, 2),        // 1: вызов подпрограммы на адресе 4, возврат в r15 = 2
        store(1, LED),                  // 2: после возврата
        HALT,                           // 3
        addi(1, 0, 9),                  // 4: подпрограмма
        reg_form(JR, 0, 15, 0),         // 5: возврат
    ];
    start_soc!(runtime, m, &program);
    run_until_halt!(m, 100);
    assert_eq!(m.led, 9, "подпрограмма изменила r1, возврат на следующую команду");
    Ok(())
}

#[test]
#[snafu::report]
fn remote_buttons_are_read_through_the_bus() -> Result<(), VerilatorError> {
    let program = [load(1, REMOTE), store(1, LED), HALT];
    for pins in 0..4u8 {
        start_soc!(runtime, m, &program);
        m.remote = pins;
        // кнопки проходят синхронизатор из двух тактов; выполнение LOAD начнётся позже
        let _ = run_until_halt!(m, 100);
        assert_eq!(m.led, pins, "кнопки {pins:#04b}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn limit_switches_and_ir_sensor_are_separate_registers() -> Result<(), VerilatorError> {
    // LED = limit | ir << 2
    let program = [
        load(1, LIMIT),
        load(2, IR_SENSOR),
        imm_form(SHLI, 2, 2, 2),
        reg_form(OR, 1, 1, 2),
        store(1, LED),
        HALT,
    ];
    for limit in 0..4u8 {
        for ir in 0..2u8 {
            start_soc!(runtime, m, &program);
            m.limit = limit;
            m.ir_sensor = ir;
            let _ = run_until_halt!(m, 100);
            assert_eq!(m.led, limit | (ir << 2), "концевики {limit:#04b}, датчик {ir}");
        }
    }
    Ok(())
}

#[test]
#[snafu::report]
fn gate_register_drives_the_motor() -> Result<(), VerilatorError> {
    // (записываемое значение, gate_open, gate_close)
    let cases = [(0u32, 0u8, 0u8), (1, 1, 0), (2, 0, 1), (3, 0, 0)];
    for (value, open, close) in cases {
        let program = [addi(1, 0, value as i32), store(1, GATE), HALT];
        start_soc!(runtime, m, &program);
        run_until_halt!(m, 100);
        assert_eq!((m.gate_open, m.gate_close), (open, close), "значение {value}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn high_res_tick_returns_the_captured_time() -> Result<(), VerilatorError> {
    let program = [
        addi(1, 0, 1),                  // 0
        store(1, CAPTURE_ARM),          // 1: взвод захвата
        imm_form(HRT, 2, 0, 0),         // 2: loop: HIGH_RES_TICK r2, Z = 1, пока захвата нет
        branch(BEQ, -2),                // 3: повторять
        store(2, LED),                  // 4: время в микросекундах на светодиоды
        HALT,                           // 5
    ];
    start_soc!(runtime, m, &program);
    // программа ждёт нажатия: захвата нет, остановки нет
    for _ in 0..500 {
        tick!(m);
        assert_eq!(m.halt, 0, "до нажатия программа не заканчивается");
    }
    assert_eq!(m.led, 0);
    // нажатие на 500-м такте после сброса: 500 / 50 = 10 микросекунд
    m.btn_react = 1;
    run_until_halt!(m, 500);
    assert_eq!(m.led, 10, "захвачено время нажатия в микросекундах");
    Ok(())
}

#[test]
#[snafu::report]
fn rust_reads_the_ram_through_the_view_device() -> Result<(), VerilatorError> {
    // удаление дубликатов не нужно: проверяем сам путь процессор -> RAM -> Rust
    let program = [
        addi(1, 0, 0x1234),
        store(1, 5),    // RAM[5] = 0x1234
        addi(2, 0, 0x5678),
        store(2, 1023), // RAM[1023] = 0x5678
        addi(3, 0, -1),
        store(3, 0),    // RAM[0] = 0xFFFFFFFF
        HALT,
    ];
    start_soc!(runtime, m, &program);
    run_until_halt!(m, 100);
    for addr in 0..1024u16 {
        m.ram_view_addr = addr;
        m.eval();
        let expected = match addr {
            0 => 0xFFFF_FFFF,
            5 => 0x1234,
            1023 => 0x5678,
            _ => 0,
        };
        assert_eq!(m.ram_view_data, expected, "слово RAM {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn cpu_writes_the_video_memory_and_rust_reads_the_frame() -> Result<(), VerilatorError> {
    let program = [
        addi(1, 0, 0x1234),
        store(1, 0x1000), // первое слово видеопамяти
        addi(2, 0, 0x5678),
        store(2, 0x1FFF), // последнее слово видеопамяти
        HALT,
    ];
    start_soc!(runtime, m, &program);
    run_until_halt!(m, 100);
    for (addr, expected) in [(0u16, 0x1234u32), (0xFFF, 0x5678), (1, 0), (0x800, 0)] {
        m.frame_addr = addr;
        m.eval();
        assert_eq!(m.frame_data, expected, "слово кадра {addr:#05x}");
    }
    Ok(())
}
