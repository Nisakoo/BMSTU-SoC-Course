//! Запуск программы на SoC в симуляции Verilator: `soc <префикс>`.
//! Префикс — имя транслированной программы без суффикса: для `build/dup` берутся
//! `build/dup.rom.hex` (команды) и `build/dup.ram.hex` (данные), см. `asm`.
//!
//! Программа выполняется до `HALT`, затем печатается итог. Если `HALT` не наступает,
//! симуляция работает, пока её не прервут (Ctrl+C).

use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// soc_top с программой из build/sim_program.hex и данными из build/sim_data.hex
#[verilog(src = "sim/soc_sim_top.v", name = "soc_sim_top")]
struct SocSimTop;

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
    "sim/soc_sim_top.v",
];

const USAGE: &str = "\
Использование: soc <префикс>

  <префикс>   транслированная программа без суффикса: для build/dup читаются
              build/dup.rom.hex и build/dup.ram.hex (их создаёт asm -o build/dup)
  -h, --help  эта справка

Программа выполняется до HALT, затем печатается итог. Если HALT не наступает,
симуляция работает, пока её не прервут (Ctrl+C).";

/// Итог остановки процессора.
struct Summary {
    ticks: u64,
    led: u8,
    gate_open: bool,
    gate_close: bool,
    /// ненулевые слова RAM: адрес и значение
    ram: Vec<(u16, u32)>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}

/// Код выхода и сообщение. 2 — неверные аргументы, 1 — остальные ошибки.
type Failure = (u8, String);

fn run(args: &[String]) -> Result<(), Failure> {
    let mut prefix: Option<&str> = None;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            flag if flag.starts_with('-') => return Err(usage_error(&format!("неизвестный флаг '{flag}'"))),
            value => {
                if prefix.replace(value).is_some() {
                    return Err(usage_error("префикс должен быть один"));
                }
            }
        }
    }
    let prefix = prefix.ok_or_else(|| usage_error("не указан префикс программы"))?;

    // пути относительно места запуска, дальше работаем из корня проекта (там rtl/, sim/, build/)
    let base = std::path::absolute(prefix).map_err(|e| (1, format!("{prefix}: {e}")))?;
    let rom = with_suffix(&base, ".rom.hex");
    let ram = with_suffix(&base, ".ram.hex");
    for file in [&rom, &ram] {
        if !file.exists() {
            return Err((
                1,
                format!(
                    "нет файла {}\nСначала соберите программу: cargo run --bin asm -- <файл.asm> -o {prefix}",
                    file.display()
                ),
            ));
        }
    }
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))
        .map_err(|e| (1, format!("{}: {e}", env!("CARGO_MANIFEST_DIR"))))?;

    let summary = simulate(&rom, &ram).map_err(|e| (1, format!("симуляция: {e}")))?;
    print_summary(&summary);
    Ok(())
}

fn usage_error(message: &str) -> Failure {
    (2, format!("{message}\n\n{USAGE}"))
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = base.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
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

/// Выполняет программу до `HALT`. Работает из корня проекта.
fn simulate(rom: &Path, ram: &Path) -> Result<Summary, Box<dyn Error>> {
    // параметры модуля из Rust задать нельзя: обёртка читает файлы с фиксированными именами
    fs::create_dir_all("build")?;
    fs::copy(rom, "build/sim_program.hex")?;
    fs::copy(ram, "build/sim_data.hex")?;

    let files: Vec<&Path> = SOC_FILES.iter().map(|f| Path::new(*f)).collect();
    let runtime = VerilatorRuntime::new(
        "build".as_ref(),
        &files,
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )?;
    let mut m = runtime.create_model_simple::<SocSimTop>()?;

    m.clk = 0;
    m.reset = 1;
    m.btn_react = 0;
    m.remote = 0;
    m.limit = 0;
    m.ir_sensor = 0;
    m.frame_addr = 0;
    m.ram_view_addr = 0;
    m.eval(); // здесь ROM и RAM читают файлы
    tick!(m);
    tick!(m);
    m.reset = 0;
    m.eval();

    let mut ticks = 0u64;
    while m.halt == 0 {
        tick!(m);
        ticks += 1;
    }

    let mut words = Vec::new();
    for addr in 0..1024u16 {
        m.ram_view_addr = addr;
        m.eval();
        if m.ram_view_data != 0 {
            words.push((addr, m.ram_view_data));
        }
    }
    Ok(Summary {
        ticks,
        led: m.led,
        gate_open: m.gate_open != 0,
        gate_close: m.gate_close != 0,
        ram: words,
    })
}

fn print_summary(summary: &Summary) {
    println!("Остановка по HALT, тактов: {}", summary.ticks);
    println!("Светодиоды: 0x{:02X} (0b{:08b})", summary.led, summary.led);
    println!(
        "Ворота: открыть = {}, закрыть = {}",
        u8::from(summary.gate_open),
        u8::from(summary.gate_close)
    );
    if summary.ram.is_empty() {
        println!("RAM: все слова нулевые");
    } else {
        println!("RAM (ненулевые слова):");
        for (addr, value) in &summary.ram {
            println!("  0x{addr:03X}  0x{value:08X}  {value}");
        }
    }
}
