use std::path::PathBuf;
use std::process::{Command, Output};

const PROGRAM: &str = ".data\n.word 5\n.text\nLI R1, 3\nHALT\n";

fn asm(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_asm"))
        .args(args)
        .output()
        .expect("запуск asm")
}

/// Пустой каталог для одного теста.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("soc_asm_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
}

#[test]
fn writes_rom_and_ram_files_named_by_the_prefix() {
    let dir = temp_dir("ok");
    let source = dir.join("p.asm");
    std::fs::write(&source, PROGRAM).unwrap();
    let prefix = dir.join("out").join("prog"); // каталога out ещё нет

    let out = asm(&[source.to_str().unwrap(), "-o", prefix.to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out.stderr));

    let rom = std::fs::read_to_string(dir.join("out").join("prog.rom.hex")).unwrap();
    let ram = std::fs::read_to_string(dir.join("out").join("prog.ram.hex")).unwrap();
    assert_eq!(rom, "@000\n44400003\n00000000\n"); // ADDI R1, R0, 3 и HALT
    assert_eq!(ram, "@000\n00000005\n");
    let stdout = text(&out.stdout);
    assert!(stdout.contains("ROM: 2 слов") && stdout.contains("RAM: 1 слов"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flag_order_does_not_matter() {
    let dir = temp_dir("order");
    let source = dir.join("p.asm");
    std::fs::write(&source, PROGRAM).unwrap();
    let prefix = dir.join("x");
    let out = asm(&["-o", prefix.to_str().unwrap(), source.to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(dir.join("x.rom.hex").exists() && dir.join("x.ram.hex").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_files_are_written_even_when_the_ram_image_is_empty() {
    let dir = temp_dir("empty");
    let source = dir.join("p.asm");
    std::fs::write(&source, "HALT\n").unwrap();
    let prefix = dir.join("only_code");
    assert!(asm(&[source.to_str().unwrap(), "-o", prefix.to_str().unwrap()]).status.success());
    assert_eq!(std::fs::read_to_string(dir.join("only_code.rom.hex")).unwrap(), "@000\n00000000\n");
    assert_eq!(std::fs::read_to_string(dir.join("only_code.ram.hex")).unwrap(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn assembly_error_names_the_file_and_the_line_and_writes_nothing() {
    let dir = temp_dir("err");
    let source = dir.join("bad.asm");
    std::fs::write(&source, "NOP\nFOO R1\n").unwrap();
    let prefix = dir.join("bad");
    let out = asm(&[source.to_str().unwrap(), "-o", prefix.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert!(stderr.contains("bad.asm") && stderr.contains("строка 2") && stderr.contains("FOO"), "{stderr}");
    assert!(!dir.join("bad.rom.hex").exists() && !dir.join("bad.ram.hex").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_input_file() {
    let dir = temp_dir("missing");
    let missing = dir.join("nope.asm");
    let out = asm(&[missing.to_str().unwrap(), "-o", dir.join("x").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("nope.asm"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn usage_errors_exit_with_code_2() {
    let cases: [(&[&str], &str); 6] = [
        (&[], "не указан входной файл"),
        (&["p.asm"], "не указан флаг -o"),
        (&["-o", "x"], "не указан входной файл"),
        (&["p.asm", "-o"], "после -o нужен префикс"),
        (&["p.asm", "q.asm", "-o", "x"], "входной файл должен быть один"),
        (&["p.asm", "-x", "-o", "x"], "неизвестный флаг '-x'"),
    ];
    for (args, message) in cases {
        let out = asm(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let stderr = text(&out.stderr);
        assert!(stderr.contains(message) && stderr.contains("Использование"), "{args:?}: {stderr}");
    }
}

#[test]
fn help() {
    for flag in ["-h", "--help"] {
        let out = asm(&[flag]);
        assert!(out.status.success());
        assert!(text(&out.stdout).contains("Использование: asm"));
    }
}
