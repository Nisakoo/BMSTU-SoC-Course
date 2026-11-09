// Запуск программы бинарником `soc` (src/main.rs): префикс -> симуляция до HALT -> итог.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::Duration;

// Запуски делят build/sim_program.hex и build/sim_data.hex: по одному за раз
static RUN_LOCK: Mutex<()> = Mutex::new(());

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("soc_run_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Собирает текст программы и пишет <dir>/prog.rom.hex и <dir>/prog.ram.hex; возвращает префикс.
fn build(dir: &Path, source: &str) -> String {
    let image = soc::assemble(source).unwrap_or_else(|e| panic!("{source:?}: {e}"));
    std::fs::write(dir.join("prog.rom.hex"), image.rom.to_hex()).unwrap();
    std::fs::write(dir.join("prog.ram.hex"), image.ram.to_hex()).unwrap();
    dir.join("prog").to_str().unwrap().to_string()
}

fn soc(args: &[&str]) -> Output {
    let _guard = RUN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::new(env!("CARGO_BIN_EXE_soc")).args(args).output().expect("запуск soc")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn runs_until_halt_and_prints_the_summary() {
    let dir = temp_dir("summary");
    let prefix = build(
        &dir,
        ".equ LED, -976
         .data
         .word 7
         .text
         LI    R1, 0xA5
         STORE R1, LED(R0)
         HALT",
    );
    let out = soc(&[&prefix]);
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "{}", text(&out.stderr));
    // LI 2 такта, STORE 3, выборка HALT 1
    assert!(stdout.contains("Остановка по HALT, тактов: 6"), "{stdout}");
    assert!(stdout.contains("Светодиоды: 0xA5 (0b10100101)"), "{stdout}");
    assert!(stdout.contains("Ворота: открыть = 0, закрыть = 0"), "{stdout}");
    assert!(stdout.contains("RAM (ненулевые слова):\n  0x000  0x00000007  7\n"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prints_gate_outputs() {
    let dir = temp_dir("gate");
    for (value, expected) in [(1, "открыть = 1, закрыть = 0"), (2, "открыть = 0, закрыть = 1")] {
        let prefix = build(&dir, &format!(".equ GATE, -960\nLI R1, {value}\nSTORE R1, GATE(R0)\nHALT"));
        let stdout = text(&soc(&[&prefix]).stdout);
        assert!(stdout.contains(expected), "{value}: {stdout}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_ram_and_immediate_halt() {
    let dir = temp_dir("halt");
    let prefix = build(&dir, "HALT");
    let stdout = text(&soc(&[&prefix]).stdout);
    assert!(stdout.contains("тактов: 1"), "{stdout}");
    assert!(stdout.contains("Светодиоды: 0x00 (0b00000000)"), "{stdout}");
    assert!(stdout.contains("RAM: все слова нулевые"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ram_words_are_listed_with_address_hex_and_decimal() {
    let dir = temp_dir("ram");
    let prefix = build(&dir, ".data\n.org 0x3FF\n.word 0xFFFFFFFF\n.org 0x10\n.word 1000\n.text\nHALT");
    let stdout = text(&soc(&[&prefix]).stdout);
    let first = stdout.find("0x010  0x000003E8  1000").unwrap_or_else(|| panic!("{stdout}"));
    let second = stdout.find("0x3FF  0xFFFFFFFF  4294967295").unwrap_or_else(|| panic!("{stdout}"));
    assert!(first < second, "адреса идут по возрастанию: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_chain_assembler_binary_then_simulation() {
    // asm asm/duplicates_to_zero.asm -o <префикс>, затем soc <префикс>
    let dir = temp_dir("chain");
    let prefix = dir.join("dup");
    let prefix = prefix.to_str().unwrap();
    let asm = Command::new(env!("CARGO_BIN_EXE_asm"))
        .args(["asm/duplicates_to_zero.asm", "-o", prefix])
        .output()
        .unwrap();
    assert!(asm.status.success(), "{}", text(&asm.stderr));

    let stdout = text(&soc(&[prefix]).stdout);
    // результат лежит с адреса 0x010: 1, 0, 2, 0, 0, 5, 0, 7, 8, 9
    let result: Vec<&str> = stdout.lines().filter(|l| l.trim_start().starts_with("0x01")).collect();
    assert_eq!(
        result,
        vec![
            "  0x010  0x00000001  1",
            "  0x012  0x00000002  2",
            "  0x015  0x00000005  5",
            "  0x017  0x00000007  7",
            "  0x018  0x00000008  8",
            "  0x019  0x00000009  9",
        ],
        "{stdout}"
    );
    assert!(stdout.contains("  0x009  0x00000009  9"), "исходный массив цел: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn works_from_another_directory_with_a_relative_prefix() {
    let dir = temp_dir("cwd");
    build(&dir, ".equ LED, -976\nLI R1, 3\nSTORE R1, LED(R0)\nHALT");
    let _guard = RUN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let out = Command::new(env!("CARGO_BIN_EXE_soc"))
        .arg("prog") // относительно текущего каталога процесса
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("Светодиоды: 0x03"), "{}", text(&out.stdout));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_program_without_halt_runs_until_it_is_interrupted() {
    let dir = temp_dir("forever");
    let _guard = RUN_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Сначала обычный запуск: на холодном кэше он собирает модель Verilator.
    // Прерывать запуск во время сборки нельзя, каталог build останется повреждённым.
    let warm = build(&dir, "HALT");
    let out = Command::new(env!("CARGO_BIN_EXE_soc")).arg(&warm).output().unwrap();
    assert!(out.status.success(), "{}", text(&out.stderr));

    let prefix = build(&dir, "loop: JUMP loop");
    let mut child = Command::new(env!("CARGO_BIN_EXE_soc"))
        .arg(&prefix)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(3));
    assert!(child.try_wait().unwrap().is_none(), "программа без HALT не должна завершаться сама");
    child.kill().unwrap();
    child.wait().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_files_are_reported_with_a_hint() {
    let dir = temp_dir("missing");
    let prefix = dir.join("nope");
    let out = soc(&[prefix.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert!(stderr.contains("nope.rom.hex") && stderr.contains("cargo run --bin asm"), "{stderr}");

    // есть ROM, нет RAM
    std::fs::write(dir.join("nope.rom.hex"), "").unwrap();
    let out = soc(&[prefix.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("nope.ram.hex"), "{}", text(&out.stderr));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn usage_errors_exit_with_code_2() {
    let cases: [(&[&str], &str); 4] = [
        (&[], "не указан префикс программы"),
        (&["a", "b"], "префикс должен быть один"),
        (&["--gui", "a"], "неизвестный флаг '--gui'"),
        (&["-x"], "неизвестный флаг '-x'"),
    ];
    for (args, message) in cases {
        let out = soc(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let stderr = text(&out.stderr);
        assert!(stderr.contains(message) && stderr.contains("Использование: soc"), "{args:?}: {stderr}");
    }
}

#[test]
fn help() {
    for flag in ["-h", "--help"] {
        let out = soc(&[flag]);
        assert!(out.status.success());
        assert!(text(&out.stdout).contains("Использование: soc <префикс>"));
    }
}
