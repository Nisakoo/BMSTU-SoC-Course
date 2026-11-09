// Командная строка бинарника `soc` (src/main.rs): аргументы и ошибки.
// Запуск с готовой программой открывает окно, поэтому здесь он не проверяется:
// окно проверяют tests/ui_test.rs, управление симуляцией — tests/sim_test.rs.
use std::process::{Command, Output};

fn soc(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_soc")).args(args).output().expect("запуск soc")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("soc_run_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn missing_files_are_reported_with_a_hint() {
    let dir = temp_dir("missing");
    let prefix = dir.join("nope");
    let out = soc(&[prefix.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert!(stderr.contains("nope.rom.hex") && stderr.contains("cargo run --bin asm"), "{stderr}");
    assert!(out.stdout.is_empty(), "в консоль ничего не выводится");

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
        assert!(out.stdout.is_empty(), "{args:?}");
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
