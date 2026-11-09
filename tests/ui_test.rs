// Окно отладки (src/ui.rs) на тестовой платформе GPUI: клики по элементам и состояние окна.
// Пиксели не рисуются, проверяются размещение элементов, обработчики и данные.
use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, ScrollDelta, ScrollWheelEvent, TestAppContext,
    TouchPhase, VisualTestContext, point, px,
};
use soc::sim::Sim;
use soc::ui::{DEFAULT_SPEED, DebugWindow, SPEEDS, memory_row_text};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Все тесты делят build/sim_program.hex и build/sim_data.hex: по одному за раз
static LOCK: Mutex<()> = Mutex::new(());

const TIMEOUT: Duration = Duration::from_secs(20);

fn start_sim(source: &str) -> Sim {
    let dir: PathBuf = std::env::temp_dir().join(format!("soc_ui_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let image = soc::assemble(source).unwrap_or_else(|e| panic!("{source:?}: {e}"));
    let rom = dir.join("p.rom.hex");
    let ram = dir.join("p.ram.hex");
    std::fs::write(&rom, image.rom.to_hex()).unwrap();
    std::fs::write(&ram, image.ram.to_hex()).unwrap();
    let sim = Sim::start(&rom, &ram).unwrap_or_else(|e| panic!("запуск: {e}"));
    let _ = std::fs::remove_dir_all(&dir);
    sim
}

fn open<'a>(cx: &'a mut TestAppContext, source: &str) -> (Entity<DebugWindow>, &'a mut VisualTestContext) {
    let sim = start_sim(source);
    cx.add_window_view(|_, cx| DebugWindow::new(sim, cx))
}

fn center(cx: &mut VisualTestContext, name: &'static str) -> Point<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("в окне нет элемента '{name}'"))
        .center()
}

fn click(cx: &mut VisualTestContext, name: &'static str) {
    let at = center(cx, name);
    cx.simulate_click(at, Modifiers::none());
}

fn mouse_down(cx: &mut VisualTestContext, name: &'static str) {
    let at = center(cx, name);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
}

fn mouse_up_on(cx: &mut VisualTestContext, name: &'static str) {
    let at = center(cx, name);
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
}

/// Даёт поток симуляции и окну время: двигает часы окна и ждёт условия.
fn settle(cx: &mut VisualTestContext, view: &Entity<DebugWindow>, what: &str, condition: impl Fn(&DebugWindow) -> bool) {
    let start = Instant::now();
    loop {
        cx.executor().advance_clock(Duration::from_millis(40));
        cx.run_until_parked();
        if view.read_with(cx, |window, _| condition(window)) {
            return;
        }
        assert!(start.elapsed() < TIMEOUT, "не дождались: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Проходит ~150 мс, чтобы убедиться, что что-то НЕ произошло.
fn idle(cx: &mut VisualTestContext) {
    for _ in 0..30 {
        cx.executor().advance_clock(Duration::from_millis(40));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn ticks(cx: &mut VisualTestContext, view: &Entity<DebugWindow>) -> u64 {
    view.read_with(cx, |window, _| window.snapshot().ticks)
}

const SIX_TICKS: &str = ".equ LED, -976
                         LI    R1, 0xA5
                         STORE R1, LED(R0)
                         HALT";
const FOREVER: &str = "loop: JUMP loop";
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

#[gpui::test]
fn layout_sensors_on_the_left_memory_on_the_right(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_view, cx) = open(cx, FOREVER);
    let sensors = cx.debug_bounds("sensors").expect("панель датчиков");
    let memory = cx.debug_bounds("memory").expect("панель памяти");
    assert!(sensors.right() <= memory.left(), "датчики слева, память справа: {sensors:?} {memory:?}");
    assert_eq!(sensors.size.width, px(300.0));
    assert!(memory.size.width > px(300.0), "память занимает остальное окно");
    assert!(sensors.size.height > px(300.0) && memory.size.height > px(300.0));
    assert_eq!(sensors.top(), memory.top(), "обе панели начинаются на одной высоте");

    // управление над обеими панелями
    let step = cx.debug_bounds("step").expect("кнопка Шаг");
    assert!(step.bottom() <= sensors.top());
    for name in ["run", "speed-0", "speed-1", "speed-2", "speed-3", "status"] {
        assert!(cx.debug_bounds(name).is_some(), "{name}");
    }
}

#[gpui::test]
fn all_sensors_and_outputs_are_on_the_screen(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_view, cx) = open(cx, FOREVER);
    let sensors = cx.debug_bounds("sensors").unwrap();
    for name in [
        "remote-open", "remote-close", "limit-open", "limit-closed", "ir", "react", "gate-open", "gate-close",
    ] {
        let bounds = cx.debug_bounds(name).unwrap_or_else(|| panic!("нет элемента {name}"));
        assert!(sensors.contains(&bounds.center()), "{name} лежит в левой панели");
    }
    // светодиоды: старший бит слева
    let mut previous = f32::MIN;
    for bit in (0..8).rev() {
        let name: &'static str = ["led-0", "led-1", "led-2", "led-3", "led-4", "led-5", "led-6", "led-7"][bit];
        let x = f32::from(cx.debug_bounds(name).unwrap_or_else(|| panic!("нет {name}")).left());
        assert!(x > previous, "{name} правее предыдущего");
        previous = x;
    }
}

#[gpui::test]
fn the_step_button_executes_one_tick(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, SIX_TICKS);
    assert_eq!(ticks(cx, &view), 0);
    for expected in 1..=3 {
        click(cx, "step");
        settle(cx, &view, "такт", |w| w.snapshot().ticks == expected);
    }
    idle(cx);
    assert_eq!(ticks(cx, &view), 3, "один щелчок — один такт");
}

#[gpui::test]
fn run_and_pause(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, FOREVER);
    click(cx, "speed-3"); // максимальная
    click(cx, "run");
    settle(cx, &view, "запуск", |w| w.snapshot().running && w.snapshot().ticks > 1000);

    click(cx, "run"); // теперь это «Пауза»
    settle(cx, &view, "пауза", |w| !w.snapshot().running);
    let paused = ticks(cx, &view);
    idle(cx);
    assert_eq!(ticks(cx, &view), paused, "на паузе такты не идут");

    click(cx, "run");
    settle(cx, &view, "возобновление", |w| w.snapshot().ticks > paused + 1000);
    click(cx, "run");
}

#[gpui::test]
fn step_is_disabled_while_running(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, FOREVER);
    click(cx, "speed-0"); // 10 тактов в секунду
    click(cx, "run");
    settle(cx, &view, "запуск", |w| w.snapshot().running);
    click(cx, "run");
    settle(cx, &view, "пауза", |w| !w.snapshot().running);
    let paused = ticks(cx, &view);

    // обработчик «Шага» на включённой кнопке работает...
    click(cx, "step");
    settle(cx, &view, "шаг на паузе", |w| w.snapshot().ticks == paused + 1);

    // ...а во время запуска кнопка выключена
    click(cx, "run");
    settle(cx, &view, "запуск", |w| w.snapshot().running);
    let before = ticks(cx, &view);
    for _ in 0..20 {
        click(cx, "step");
    }
    idle(cx);
    // 10 тактов/с: за 0,15 с не больше пары тактов; двадцать «шагов» дали бы больше 20
    assert!(ticks(cx, &view) - before < 15, "шаг во время запуска не должен срабатывать");
    click(cx, "run");
}

#[gpui::test]
fn run_and_step_are_disabled_after_halt(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, "HALT");
    click(cx, "step");
    settle(cx, &view, "остановка", |w| w.snapshot().halted);
    assert_eq!(ticks(cx, &view), 1);
    click(cx, "step");
    click(cx, "run");
    idle(cx);
    let snapshot = view.read_with(cx, |w, _| w.snapshot().clone());
    assert_eq!(snapshot.ticks, 1);
    assert!(snapshot.halted && !snapshot.running);
}

#[gpui::test]
fn the_speed_buttons_select_the_speed(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, FOREVER);
    assert_eq!(view.read_with(cx, |w, _| w.speed_index()), DEFAULT_SPEED);
    assert_eq!(SPEEDS.len(), 4);
    for (index, name) in ["speed-0", "speed-1", "speed-2", "speed-3"].into_iter().enumerate() {
        click(cx, name);
        assert_eq!(view.read_with(cx, |w, _| w.speed_index()), index, "{name}");
    }
    // выбранная скорость действует: максимальная намного быстрее 10 тактов/с
    click(cx, "speed-0");
    click(cx, "run");
    idle(cx);
    let slow = ticks(cx, &view);
    assert!(slow < 20, "10 тактов/с: {slow}");
    click(cx, "speed-3");
    settle(cx, &view, "ускорение", |w| w.snapshot().ticks > slow + 5000);
    click(cx, "run");
}

#[gpui::test]
fn momentary_buttons_are_pressed_only_while_held(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, FOREVER);
    let inputs = |cx: &mut VisualTestContext| view.read_with(cx, |w, _| w.inputs());

    mouse_down(cx, "remote-open");
    assert_eq!(inputs(cx).remote, 0b01);
    mouse_up_on(cx, "remote-open");
    assert_eq!(inputs(cx).remote, 0);

    mouse_down(cx, "remote-close");
    assert_eq!(inputs(cx).remote, 0b10, "кнопки пульта независимы: нажата только «Закрыть»");
    mouse_up_on(cx, "remote-close");
    assert_eq!(inputs(cx).remote, 0);

    // у мыши одна кнопка: отпускание над соседней кнопкой отпускает нажатую
    mouse_down(cx, "remote-close");
    mouse_up_on(cx, "remote-open");
    assert_eq!(inputs(cx).remote, 0);

    mouse_down(cx, "react");
    assert!(inputs(cx).btn_react);
    mouse_up_on(cx, "react");
    assert!(!inputs(cx).btn_react);
}

#[gpui::test]
fn releasing_the_mouse_outside_the_button_releases_it(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, FOREVER);
    for (name, check) in [
        ("remote-open", (|w: &DebugWindow| w.inputs().remote & 1 != 0) as fn(&DebugWindow) -> bool),
        ("remote-close", |w| w.inputs().remote & 2 != 0),
        ("react", |w| w.inputs().btn_react),
    ] {
        mouse_down(cx, name);
        assert!(view.read_with(cx, |w, _| check(w)), "{name} нажата");
        // кнопка мыши отпущена в стороне от элемента
        let outside = center(cx, "memory");
        cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
        assert!(!view.read_with(cx, |w, _| check(w)), "{name} отпущена");
    }
}

#[gpui::test]
fn switches_keep_their_position(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, FOREVER);
    let inputs = |cx: &mut VisualTestContext| view.read_with(cx, |w, _| w.inputs());

    click(cx, "limit-open");
    assert_eq!(inputs(cx).limit, 0b01);
    idle(cx);
    assert_eq!(inputs(cx).limit, 0b01, "остаётся включённым");
    click(cx, "limit-closed");
    assert_eq!(inputs(cx).limit, 0b11);
    click(cx, "limit-open");
    assert_eq!(inputs(cx).limit, 0b10);
    click(cx, "limit-closed");
    assert_eq!(inputs(cx).limit, 0);

    click(cx, "ir");
    assert!(inputs(cx).ir_sensor);
    click(cx, "ir");
    assert!(!inputs(cx).ir_sensor);
    // переключатели не затрагивают другие входы
    assert_eq!(inputs(cx), soc::sim::Inputs::default());
}

#[gpui::test]
fn sensors_in_the_window_reach_the_processor_and_the_leds_show_it(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, SENSORS_TO_LEDS);
    click(cx, "speed-3");
    click(cx, "run");
    let led = |cx: &mut VisualTestContext| view.read_with(cx, |w, _| w.snapshot().led);

    mouse_down(cx, "remote-open");
    settle(cx, &view, "пульт: открыть", |w| w.snapshot().led == 0b00001);
    mouse_up_on(cx, "remote-open");
    settle(cx, &view, "пульт отпущен", |w| w.snapshot().led == 0);

    click(cx, "limit-closed");
    settle(cx, &view, "концевик «закрыто»", |w| w.snapshot().led == 0b01000);
    click(cx, "ir");
    settle(cx, &view, "ИК-датчик", |w| w.snapshot().led == 0b11000);
    click(cx, "limit-closed");
    click(cx, "ir");
    settle(cx, &view, "всё выключено", |w| w.snapshot().led == 0);
    assert_eq!(led(cx), 0);
    click(cx, "run");
}

#[gpui::test]
fn the_status_and_ticks_follow_the_simulation(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (view, cx) = open(cx, SIX_TICKS);
    click(cx, "speed-3");
    click(cx, "run");
    settle(cx, &view, "HALT", |w| w.snapshot().halted);
    let snapshot = view.read_with(cx, |w, _| w.snapshot().clone());
    assert_eq!(snapshot.ticks, 6);
    assert_eq!(snapshot.led, 0xA5);
}

#[gpui::test]
fn the_memory_list_shows_the_first_rows_and_scrolls_to_the_last(cx: &mut TestAppContext) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_view, cx) = open(cx, ".data\n.word 1, 2, 3, 4\n.text\nHALT");
    assert!(cx.debug_bounds("ram-row-0").is_some(), "первая строка видна");
    assert!(cx.debug_bounds("ram-row-255").is_none(), "последняя строка за экраном");

    let at = center(cx, "ram-row-0");
    cx.simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-1_000_000.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("ram-row-255").is_some(), "после прокрутки видна последняя строка");
    assert!(cx.debug_bounds("ram-row-0").is_none());
}

#[test]
fn memory_rows_are_formatted_as_hex_with_the_word_address() {
    let mut ram = vec![0u32; 1024];
    ram[0] = 1;
    ram[1] = 0xDEADBEEF;
    ram[7] = 0xFFFF_FFFF;
    ram[1023] = 0x1234;
    assert_eq!(
        memory_row_text(0, &ram),
        ("000".to_string(), vec!["00000001".into(), "DEADBEEF".into(), "00000000".into(), "00000000".into()])
    );
    assert_eq!(memory_row_text(1, &ram).0, "004");
    assert_eq!(memory_row_text(1, &ram).1[3], "FFFFFFFF");
    assert_eq!(memory_row_text(16, &ram).0, "040");
    assert_eq!(memory_row_text(255, &ram).0, "3FC");
    assert_eq!(memory_row_text(255, &ram).1[3], "00001234");
}
