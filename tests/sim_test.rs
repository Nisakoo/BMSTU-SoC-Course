// Управление симуляцией: поток с моделью, команды, снимки состояния (src/sim.rs).
use soc::sim::{Inputs, Sim, Snapshot, Speed, RAM_WORDS};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Все тесты делят build/sim_program.hex и build/sim_data.hex: по одному за раз
static LOCK: Mutex<()> = Mutex::new(());

const TIMEOUT: Duration = Duration::from_secs(20);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("soc_sim_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Собирает программу и запускает симуляцию.
fn start(name: &str, source: &str) -> Sim {
    let dir = temp_dir(name);
    let image = soc::assemble(source).unwrap_or_else(|e| panic!("{source:?}: {e}"));
    let rom = dir.join("p.rom.hex");
    let ram = dir.join("p.ram.hex");
    std::fs::write(&rom, image.rom.to_hex()).unwrap();
    std::fs::write(&ram, image.ram.to_hex()).unwrap();
    let sim = Sim::start(&rom, &ram).unwrap_or_else(|e| panic!("запуск: {e}"));
    let _ = std::fs::remove_dir_all(&dir);
    sim
}

/// Ждёт, пока снимок не удовлетворит условию; возвращает его.
fn wait_for(sim: &Sim, what: &str, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let start = Instant::now();
    loop {
        let snapshot = sim.snapshot();
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(start.elapsed() < TIMEOUT, "не дождались: {what}; последний снимок: ticks = {}, led = {:#04x}, halted = {}", snapshot.ticks, snapshot.led, snapshot.halted);
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Один такт и ожидание его результата.
fn step(sim: &Sim, expected_ticks: u64) -> Snapshot {
    sim.step();
    wait_for(sim, &format!("такт {expected_ticks}"), |s| s.ticks == expected_ticks)
}

const SIX_TICKS: &str = ".equ LED, -976
                         LI    R1, 0xA5
                         STORE R1, LED(R0)
                         HALT";

const FOREVER: &str = "loop: JUMP loop";

#[test]
fn starts_in_the_initial_state() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("initial", ".data\n.word 7, 8\n.text\nHALT");
    let snapshot = sim.snapshot();
    assert_eq!(snapshot.ticks, 0);
    assert!(!snapshot.running && !snapshot.halted);
    assert_eq!(snapshot.led, 0);
    assert!(!snapshot.gate_open && !snapshot.gate_close);
    assert_eq!(snapshot.ram.len(), RAM_WORDS);
    assert_eq!(&snapshot.ram[..3], &[7, 8, 0]);
    assert!(snapshot.ram[3..].iter().all(|&w| w == 0));
}

#[test]
fn step_executes_exactly_one_tick() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("step", SIX_TICKS);
    // LI 2 такта, STORE 3, выборка HALT 1
    for tick in 1..=5 {
        let snapshot = step(&sim, tick);
        assert!(!snapshot.halted, "такт {tick}");
        assert!(!snapshot.running);
    }
    let snapshot = step(&sim, 6);
    assert!(snapshot.halted);
    assert_eq!(snapshot.led, 0xA5);
}

#[test]
fn step_changes_the_state_one_tick_at_a_time() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("granular", SIX_TICKS);
    let mut led_seen = Vec::new();
    for tick in 1..=6 {
        led_seen.push(step(&sim, tick).led);
    }
    // светодиоды меняются один раз, в такте записи STORE, а не сразу
    assert_eq!(led_seen[0], 0, "после первого такта выборки светодиоды не горят");
    assert_eq!(led_seen[1], 0);
    assert_eq!(led_seen[5], 0xA5);
    let changes = led_seen.windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(changes, 1, "{led_seen:?}");
}

#[test]
fn step_after_halt_does_nothing() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("after_halt", "HALT");
    let snapshot = step(&sim, 1);
    assert!(snapshot.halted);
    sim.step();
    sim.step();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(sim.snapshot().ticks, 1);
}

#[test]
fn run_goes_to_halt_and_stops() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("run", SIX_TICKS);
    sim.run();
    let snapshot = wait_for(&sim, "остановка по HALT", |s| s.halted);
    assert_eq!(snapshot.ticks, 6);
    assert!(!snapshot.running);
    assert_eq!(snapshot.led, 0xA5);
    // после остановки состояние не меняется
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(sim.snapshot().ticks, 6);
    sim.run();
    std::thread::sleep(Duration::from_millis(50));
    let after = sim.snapshot();
    assert!(!after.running && after.ticks == 6);
}

#[test]
fn pause_stops_a_running_program_and_run_resumes_it() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("pause", FOREVER);
    sim.run();
    wait_for(&sim, "такты идут", |s| s.running && s.ticks > 1000);
    sim.pause();
    let paused = wait_for(&sim, "пауза", |s| !s.running);
    std::thread::sleep(Duration::from_millis(150));
    let later = sim.snapshot();
    assert_eq!(later.ticks, paused.ticks, "во время паузы такты не идут");
    assert!(!later.halted);

    sim.run();
    wait_for(&sim, "возобновление", |s| s.running && s.ticks > paused.ticks + 1000);
    sim.pause();
}

#[test]
fn speed_limits_the_ticks_per_second() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("speed", FOREVER);
    sim.set_speed(Speed::PerSecond(200));
    sim.run();
    std::thread::sleep(Duration::from_millis(500));
    let ticks = sim.snapshot().ticks;
    sim.pause();
    // 200 тактов/с за полсекунды — около 100; без ограничения были бы десятки тысяч
    assert!((20..=300).contains(&ticks), "тактов за 0,5 с: {ticks}");
}

#[test]
fn speed_can_be_changed_while_running() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("speed_change", FOREVER);
    sim.set_speed(Speed::PerSecond(10));
    sim.run();
    std::thread::sleep(Duration::from_millis(200));
    assert!(sim.snapshot().ticks < 100);
    sim.set_speed(Speed::Max);
    wait_for(&sim, "ускорение", |s| s.ticks > 5000);
    sim.pause();
}

#[test]
fn a_slow_run_takes_about_the_expected_time() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("slow_halt", SIX_TICKS);
    sim.set_speed(Speed::PerSecond(100));
    let started = Instant::now();
    sim.run();
    wait_for(&sim, "остановка", |s| s.halted);
    let elapsed = started.elapsed();
    // 6 тактов при 100 тактов/с: около 60 мс
    assert!(elapsed >= Duration::from_millis(40), "слишком быстро: {elapsed:?}");
    assert!(elapsed < Duration::from_secs(5), "слишком медленно: {elapsed:?}");
}

#[test]
fn snapshot_version_grows_on_every_publication() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("version", SIX_TICKS);
    let first = sim.version();
    step(&sim, 1);
    let second = sim.version();
    assert!(second > first);
    step(&sim, 2);
    assert!(sim.version() > second);
    // без команд версия стоит на месте
    let stable = sim.version();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(sim.version(), stable);
    assert_eq!(sim.snapshot().version, stable);
}

#[test]
fn the_snapshot_shows_ram_written_by_the_program() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("ram", "LI R1, 0x1234\nSTORE R1, 5(R0)\nSTORE R1, 1023(R0)\nHALT");
    sim.run();
    let snapshot = wait_for(&sim, "остановка", |s| s.halted);
    assert_eq!(snapshot.ram[5], 0x1234);
    assert_eq!(snapshot.ram[1023], 0x1234);
    assert_eq!(snapshot.ram.iter().filter(|&&w| w != 0).count(), 2);
}

#[test]
fn gate_outputs_are_in_the_snapshot() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (value, open, close) in [(1, true, false), (2, false, true), (3, false, false)] {
        let sim = start("gate", &format!(".equ GATE, -960\nLI R1, {value}\nSTORE R1, GATE(R0)\nHALT"));
        sim.run();
        let snapshot = wait_for(&sim, "остановка", |s| s.halted);
        assert_eq!((snapshot.gate_open, snapshot.gate_close), (open, close), "значение {value}");
    }
}

// Программа читает пульт, концевики и ИК-датчик и показывает их на светодиодах:
// бит 0–1 — пульт, 2–3 — концевики, 4 — ИК-датчик
const SENSORS_TO_LEDS: &str = ".equ REMOTE, -992
        .equ LIMIT, -991
        .equ IR, -990
        .equ LED, -976
loop:   LOAD  R1, REMOTE(R0)
        LOAD  R2, LIMIT(R0)
        SHLI  R2, R2, 2
        OR    R1, R1, R2
        LOAD  R3, IR(R0)
        SHLI  R3, R3, 4
        OR    R1, R1, R3
        STORE R1, LED(R0)
        JUMP  loop";

#[test]
fn inputs_reach_the_processor() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("inputs", SENSORS_TO_LEDS);
    sim.run();
    let cases = [
        (Inputs { remote: 0b01, ..Inputs::default() }, 0b00001),
        (Inputs { remote: 0b10, ..Inputs::default() }, 0b00010),
        (Inputs { limit: 0b01, ..Inputs::default() }, 0b00100),
        (Inputs { limit: 0b10, ..Inputs::default() }, 0b01000),
        (Inputs { ir_sensor: true, ..Inputs::default() }, 0b10000),
        (Inputs { remote: 0b11, limit: 0b11, ir_sensor: true, btn_react: false }, 0b11111),
        (Inputs::default(), 0),
    ];
    for (inputs, expected) in cases {
        sim.set_inputs(inputs);
        wait_for(&sim, &format!("светодиоды {expected:#07b} при {inputs:?}"), |s| s.led == expected);
    }
}

#[test]
fn inputs_can_be_changed_while_paused_and_are_seen_after_stepping() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("inputs_paused", SENSORS_TO_LEDS);
    sim.set_inputs(Inputs { remote: 0b01, ..Inputs::default() });
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(sim.snapshot().led, 0, "без тактов светодиоды не меняются");
    for tick in 1..=60 {
        step(&sim, tick);
    }
    assert_eq!(sim.snapshot().led, 1);
}

#[test]
fn the_reaction_button_is_captured_by_the_program() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // взвести захват и ждать нажатия; по HIGH_RES_TICK зажечь все светодиоды и остановиться
    let sim = start(
        "reaction",
        ".equ CAPTURE_ARM, -1008
         .equ LED, -976
                 STORE R0, CAPTURE_ARM(R0)
         wait:   HIGH_RES_TICK R1
                 BEQ   wait
                 LI    R2, 0xFF
                 STORE R2, LED(R0)
                 HALT",
    );
    sim.run();
    std::thread::sleep(Duration::from_millis(200));
    let waiting = sim.snapshot();
    assert!(waiting.running && !waiting.halted && waiting.led == 0, "ждёт нажатия");

    sim.set_inputs(Inputs { btn_react: true, ..Inputs::default() });
    let done = wait_for(&sim, "захват по нажатию", |s| s.halted);
    assert_eq!(done.led, 0xFF);
}

#[test]
fn the_reaction_button_pressed_and_released_before_the_program_arms_is_ignored() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start(
        "reaction_early",
        ".equ CAPTURE_ARM, -1008
         .equ LED, -976
                 LI    R5, 200
         delay:  SUBI  R5, R5, 1
                 BNE   delay
                 STORE R0, CAPTURE_ARM(R0)
         wait:   HIGH_RES_TICK R1
                 BEQ   wait
                 LI    R2, 0xFF
                 STORE R2, LED(R0)
                 HALT",
    );
    // кнопка нажата и отпущена до взвода захвата
    sim.set_inputs(Inputs { btn_react: true, ..Inputs::default() });
    for tick in 1..=10 {
        step(&sim, tick);
    }
    sim.set_inputs(Inputs::default());
    sim.run();
    std::thread::sleep(Duration::from_millis(300));
    let snapshot = sim.snapshot();
    assert!(!snapshot.halted && snapshot.led == 0, "ранее нажатие не должно засчитываться");
}

#[test]
fn start_reports_a_missing_file() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("missing");
    let rom = dir.join("nope.rom.hex");
    let ram = dir.join("nope.ram.hex");
    let error = Sim::start(&rom, &ram).err().expect("ошибка");
    assert!(error.contains("nope.rom.hex"), "{error}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropping_the_simulation_stops_its_thread_even_when_running() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sim = start("drop", FOREVER);
    sim.run();
    wait_for(&sim, "такты идут", |s| s.ticks > 100);
    let started = Instant::now();
    drop(sim);
    assert!(started.elapsed() < Duration::from_secs(2), "поток не остановился");
    // можно сразу запустить другую симуляцию
    let sim = start("drop_again", "HALT");
    assert_eq!(sim.snapshot().ticks, 0);
}
