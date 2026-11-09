// Программы, собранные ассемблером, выполняются на soc_top (Verilator).
// Образы пишутся в build/asm_program.hex и build/asm_data.hex: tests/rtl/soc_asm_top.v.
use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

#[verilog(src = "tests/rtl/soc_asm_top.v", name = "soc_asm_top")]
struct SocAsmTop;

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
    "tests/rtl/soc_asm_top.v",
];

// ROM и RAM читают файлы при первом eval(): файлы не должны меняться, пока модель не создана
static FILES_LOCK: Mutex<()> = Mutex::new(());

macro_rules! tick {
    ($m:expr) => {{
        $m.clk = 0;
        $m.eval();
        $m.clk = 1;
        $m.eval();
    }};
}

// Создаёт модель с образами из текстов hex-файлов и снимает сброс
macro_rules! start_soc {
    ($runtime:ident, $m:ident, $rom_hex:expr, $ram_hex:expr) => {
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
            let _guard = FILES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            std::fs::create_dir_all("build").expect("каталог build");
            std::fs::write("build/asm_program.hex", $rom_hex).expect("образ ROM");
            std::fs::write("build/asm_data.hex", $ram_hex).expect("образ RAM");
            let mut m = $runtime.create_model_simple::<SocAsmTop>()?;
            m.clk = 0;
            m.reset = 1;
            m.btn_react = 0;
            m.remote = 0;
            m.limit = 0;
            m.ir_sensor = 0;
            m.frame_addr = 0;
            m.ram_view_addr = 0;
            m.eval(); // здесь ROM и RAM читают файлы
            m
        };
        tick!($m);
        tick!($m);
        $m.reset = 0;
        $m.eval();
    };
}

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

macro_rules! ram_word {
    ($m:expr, $addr:expr) => {{
        $m.ram_view_addr = $addr;
        $m.eval();
        $m.ram_view_data
    }};
}

#[test]
#[snafu::report]
fn duplicates_program_from_the_file_through_the_command_line() -> Result<(), VerilatorError> {
    // файл -> asm (командная строка) -> hex-файлы -> процессор
    let prefix = format!("build/asm_cli_dup_{}", std::process::id());
    let out = Command::new(env!("CARGO_BIN_EXE_asm"))
        .args(["asm/duplicates_to_zero.asm", "-o", &prefix])
        .output()
        .expect("запуск asm");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let rom_hex = std::fs::read_to_string(format!("{prefix}.rom.hex")).unwrap();
    let ram_hex = std::fs::read_to_string(format!("{prefix}.ram.hex")).unwrap();
    let _ = std::fs::remove_file(format!("{prefix}.rom.hex"));
    let _ = std::fs::remove_file(format!("{prefix}.ram.hex"));

    // исходный массив берём из образа: слова данных до адреса результата (res = 16)
    let image = soc::assemble(&std::fs::read_to_string("asm/duplicates_to_zero.asm").unwrap()).unwrap();
    let array: Vec<u32> = (0..16).map_while(|a| image.ram.get(a)).collect();
    assert!(array.len() >= 2, "в программе нет массива в начале RAM");

    start_soc!(runtime, m, rom_hex, ram_hex);
    run_until_halt!(m, 10_000);

    let mut expected = Vec::new();
    for (i, &value) in array.iter().enumerate() {
        expected.push(if i > 0 && array[i - 1] == value { 0 } else { value });
    }
    for (i, &value) in array.iter().enumerate() {
        assert_eq!(ram_word!(m, i as u16), value, "массив изменился: arr[{i}]");
        assert_eq!(ram_word!(m, 16 + i as u16), expected[i], "результат res[{i}]");
    }
    assert_eq!(ram_word!(m, 16 + array.len() as u16), 0, "за результатом ничего не записано");
    Ok(())
}

#[test]
#[snafu::report]
fn subroutine_shifts_and_branches_give_the_expected_results() -> Result<(), VerilatorError> {
    let image = soc::assemble(
        "       .data
                .org 4
        res:    .word 0, 0, 0, 0
                .text
                LI    R4, res          ; база результатов
                LI    R1, 7
                JAL   R15, square      ; R2 = 49
                STORE R2, 0(R4)
                LI    R3, 1
                SHLI  R3, R3, 4        ; 16
                MOV   R5, R3
                SUBI  R5, R5, 1        ; 15
                XORI  R5, R5, 0xFF     ; 0xF0
                NOP
                STORE R5, 1(R4)
                LI    R6, -8
                SARI  R6, R6, 1        ; -4
                STORE R6, 2(R4)
                CMPI  R1, 7
                BEQ   equal
                LI    R7, 111
                JUMP  done
        equal:  LI    R7, 222
        done:   STORE R7, 3(R4)
                HALT
        square: LI    R2, 0            ; R2 = R1 * R1 сложением
                MOV   R3, R1
        again:  ADD   R2, R2, R1
                SUBI  R3, R3, 1
                BNE   again
                JR    R15",
    )
    .unwrap();

    start_soc!(runtime, m, image.rom.to_hex(), image.ram.to_hex());
    run_until_halt!(m, 10_000);
    assert_eq!(ram_word!(m, 4), 49);
    assert_eq!(ram_word!(m, 5), 0xF0);
    assert_eq!(ram_word!(m, 6), 0xFFFF_FFFC);
    assert_eq!(ram_word!(m, 7), 222);
    assert_eq!(ram_word!(m, 8), 0, "за результатами ничего не записано");
    Ok(())
}

#[test]
#[snafu::report]
fn mmio_names_from_equ_drive_the_leds() -> Result<(), VerilatorError> {
    let image = soc::assemble(
        ".equ LED, -976
         .equ TIMER, -1024
                LI    R1, 0xA5
                STORE R1, LED(R0)
                LOAD  R2, TIMER(R0)
                HALT",
    )
    .unwrap();
    start_soc!(runtime, m, image.rom.to_hex(), image.ram.to_hex());
    run_until_halt!(m, 1000);
    assert_eq!(m.led, 0xA5);
    Ok(())
}
