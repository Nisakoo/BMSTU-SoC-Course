//! Запуск программы на SoC в симуляции Verilator с окном отладки: `soc <префикс>`.
//! Префикс — имя транслированной программы без суффикса: для `build/dup` берутся
//! `build/dup.rom.hex` (команды) и `build/dup.ram.hex` (данные), см. `asm`.

use soc::sim::Sim;
use soc::ui;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
Использование: soc <префикс>

  <префикс>   транслированная программа без суффикса: для build/dup читаются
              build/dup.rom.hex и build/dup.ram.hex (их создаёт asm -o build/dup)
  -h, --help  эта справка

Открывается окно отладки: датчики и светодиоды слева, память справа,
управление тактами сверху.";

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

    let sim = Sim::start(&rom, &ram).map_err(|e| (1, format!("симуляция: {e}")))?;
    ui::run(sim);
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
