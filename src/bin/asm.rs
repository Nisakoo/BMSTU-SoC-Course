//! Командная строка ассемблера: `asm файл.asm -o префикс`.
//! Пишет два файла: `префикс.rom.hex` (команды) и `префикс.ram.hex` (данные).

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
Использование: asm <файл.asm> -o <префикс>

  -o <префикс>   пишет <префикс>.rom.hex (ROM команд) и <префикс>.ram.hex (RAM данных)
  -h, --help     эта справка";

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

/// Ошибка: код выхода и сообщение. 2 — неверные аргументы, 1 — ошибка программы или файлов.
type Failure = (u8, String);

fn run(args: &[String]) -> Result<(), Failure> {
    let mut input: Option<&str> = None;
    let mut prefix: Option<&str> = None;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-o" => match iter.next() {
                Some(value) => prefix = Some(value),
                None => return Err(usage_error("после -o нужен префикс")),
            },
            flag if flag.starts_with('-') => return Err(usage_error(&format!("неизвестный флаг '{flag}'"))),
            file => {
                if input.replace(file).is_some() {
                    return Err(usage_error("входной файл должен быть один"));
                }
            }
        }
    }
    let input = input.ok_or_else(|| usage_error("не указан входной файл"))?;
    let prefix = prefix.ok_or_else(|| usage_error("не указан флаг -o"))?;

    let text = fs::read_to_string(input).map_err(|e| (1, format!("{input}: {e}")))?;
    let image = soc::assemble(&text).map_err(|e| (1, format!("{input}: {e}")))?;

    let rom_path = format!("{prefix}.rom.hex");
    let ram_path = format!("{prefix}.ram.hex");
    write_file(&rom_path, &image.rom.to_hex())?;
    write_file(&ram_path, &image.ram.to_hex())?;
    println!("ROM: {} слов -> {rom_path}", image.rom.len());
    println!("RAM: {} слов -> {ram_path}", image.ram.len());
    Ok(())
}

fn usage_error(message: &str) -> Failure {
    (2, format!("{message}\n\n{USAGE}"))
}

fn write_file(path: &str, text: &str) -> Result<(), Failure> {
    if let Some(dir) = Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(|e| (1, format!("{}: {e}", dir.display())))?;
    }
    fs::write(path, text).map_err(|e| (1, format!("{path}: {e}")))
}
