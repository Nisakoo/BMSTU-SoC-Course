//! Окно отладки на GPUI: слева датчики и выводы, справа память, сверху управление тактами.

use gpui::{
    App, Application, Bounds, Context, MouseButton, SharedString, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size, uniform_list,
};
use crate::sim::{Inputs, RAM_WORDS, Sim, Snapshot, Speed};
use std::time::Duration;

// Светлая тема
const BACKGROUND: u32 = 0xFFFFFF;
const PANEL: u32 = 0xF6F8FA;
const BORDER: u32 = 0xD0D7DE;
const TEXT: u32 = 0x1F2328;
const MUTED: u32 = 0x6E7781;
const DIM: u32 = 0xB6BEC7;
const BUTTON: u32 = 0xF3F4F6;
const BUTTON_PRESSED: u32 = 0xB6D4FE;
const ACCENT: u32 = 0x0969DA;
const LED_ON: u32 = 0xE5484D;
const LED_OFF: u32 = 0xE6E8EB;
const LAMP_ON: u32 = 0x2DA44E;

const LEFT_PANEL_WIDTH: f32 = 300.0;
/// Слов в строке просмотра памяти.
const WORDS_PER_ROW: usize = 4;
const ROWS: usize = RAM_WORDS / WORDS_PER_ROW;

/// Кнопки скорости: подпись и скорость.
pub const SPEEDS: [(&str, Speed); 4] = [
    ("10 / с", Speed::PerSecond(10)),
    ("1 000 / с", Speed::PerSecond(1_000)),
    ("100 000 / с", Speed::PerSecond(100_000)),
    ("Макс.", Speed::Max),
];
/// Номер скорости в `SPEEDS`, выбранной при открытии окна.
pub const DEFAULT_SPEED: usize = 1;

/// Открывает окно и работает, пока оно не закрыто.
pub fn run(sim: Sim) {
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1000.0), px(660.0)), cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("SoC: отладка".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| DebugWindow::new(sim, cx)),
        )
        .expect("не удалось открыть окно");
        cx.activate(true);
    });
}

pub struct DebugWindow {
    sim: Sim,
    snapshot: Snapshot,
    inputs: Inputs,
    speed: usize,
}

impl DebugWindow {
    pub fn new(sim: Sim, cx: &mut Context<Self>) -> Self {
        sim.set_speed(SPEEDS[DEFAULT_SPEED].1);
        // окно перерисовывается, когда поток симуляции опубликовал новый снимок
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        DebugWindow {
            snapshot: sim.snapshot(),
            sim,
            inputs: Inputs::default(),
            speed: DEFAULT_SPEED,
        }
    }

    /// Последний снимок, который показывает окно.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Состояние входов, выставленное из окна.
    pub fn inputs(&self) -> Inputs {
        self.inputs
    }

    /// Номер выбранной скорости в `SPEEDS`.
    pub fn speed_index(&self) -> usize {
        self.speed
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.sim.version() != self.snapshot.version {
            self.snapshot = self.sim.snapshot();
            cx.notify();
        }
    }

    fn change_inputs(&mut self, change: impl FnOnce(&mut Inputs), cx: &mut Context<Self>) {
        change(&mut self.inputs);
        self.sim.set_inputs(self.inputs);
        cx.notify();
    }

    fn toggle_run(&mut self, cx: &mut Context<Self>) {
        if self.snapshot.running {
            self.sim.pause();
        } else {
            self.sim.run();
        }
        cx.notify();
    }

    fn select_speed(&mut self, index: usize, cx: &mut Context<Self>) {
        self.speed = index;
        self.sim.set_speed(SPEEDS[index].1);
        cx.notify();
    }
}

impl Render for DebugWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .text_sm()
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(self.sensors_panel(cx))
                    .child(self.memory_panel(cx)),
            )
    }
}

// ---------- управление ----------

impl DebugWindow {
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let halted = self.snapshot.halted;
        let running = self.snapshot.running;

        let step = push_button(
            "step".to_string(),
            "Шаг",
            !halted && !running,
            false,
            cx.listener(|this, _, _, _| this.sim.step()),
        );
        let run = push_button(
            "run".to_string(),
            if running { "Пауза" } else { "Запуск" },
            !halted,
            false,
            cx.listener(|this, _, _, cx| this.toggle_run(cx)),
        );

        let mut speeds = div().flex().gap_1();
        for (index, (label, _)) in SPEEDS.iter().enumerate() {
            speeds = speeds.child(
                push_button(
                    format!("speed-{index}"),
                    label,
                    true,
                    index == self.speed,
                    cx.listener(move |this, _, _, cx| this.select_speed(index, cx)),
                ),
            );
        }

        let status = if halted {
            "HALT"
        } else if running {
            "работает"
        } else {
            "остановлен"
        };

        div()
            .flex()
            .items_center()
            .gap_4()
            .px_3()
            .py_2()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(div().flex().gap_1().child(step).child(run))
            .child(speeds)
            .child(div().text_color(rgb(MUTED)).child("такт:"))
            .child(
                div()
                    .font_family("Menlo")
                    .min_w(px(90.0))
                    .child(format!("{}", self.snapshot.ticks)),
            )
            .child(
                div()
                    .debug_selector(|| "status".to_string())
                    .text_color(rgb(if halted { LED_ON } else { MUTED }))
                    .child(status),
            )
    }
}

// ---------- датчики и выводы ----------

impl DebugWindow {
    fn sensors_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let inputs = self.inputs;

        let remote = section("Пульт")
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.hold_button("remote-open", "Открыть", inputs.remote & 0b01 != 0, cx, |i, v| set_bit(&mut i.remote, 0, v)))
                    .child(self.hold_button("remote-close", "Закрыть", inputs.remote & 0b10 != 0, cx, |i, v| set_bit(&mut i.remote, 1, v))),
            );

        let limits = section("Концевые выключатели")
            .child(self.switch("limit-open", "Открыто", inputs.limit & 0b01 != 0, cx, |i| i.limit ^= 0b01))
            .child(self.switch("limit-closed", "Закрыто", inputs.limit & 0b10 != 0, cx, |i| i.limit ^= 0b10));

        let ir = section("ИК-датчик").child(self.switch(
            "ir",
            "Препятствие",
            inputs.ir_sensor,
            cx,
            |i| i.ir_sensor = !i.ir_sensor,
        ));

        let reaction = section("Кнопка реакции").child(div().flex().child(self.hold_button(
            "react",
            "Нажать",
            inputs.btn_react,
            cx,
            |i, v| i.btn_react = v,
        )));

        let mut leds = div().flex().gap_2();
        for bit in (0..8).rev() {
            let on = self.snapshot.led >> bit & 1 != 0;
            leds = leds.child(
                div()
                    .debug_selector(move || format!("led-{bit}"))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(lamp(on, LED_ON, 22.0))
                    .child(div().text_xs().text_color(rgb(MUTED)).child(format!("{bit}"))),
            );
        }
        let led_section = section("Светодиоды").child(leds);

        let gate = section("Привод ворот").child(
            div()
                .flex()
                .gap_4()
                .child(labeled_lamp("gate-open", "Открыть", self.snapshot.gate_open))
                .child(labeled_lamp("gate-close", "Закрыть", self.snapshot.gate_close)),
        );

        div()
            .debug_selector(|| "sensors".to_string())
            .w(px(LEFT_PANEL_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_3()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(heading("Датчики"))
            .child(remote)
            .child(limits)
            .child(ir)
            .child(reaction)
            .child(heading("Выводы"))
            .child(led_section)
            .child(gate)
    }

    /// Кнопка, нажатая, пока держат кнопку мыши.
    fn hold_button(
        &self,
        id: &'static str,
        label: &'static str,
        pressed: bool,
        cx: &mut Context<Self>,
        set: fn(&mut Inputs, bool),
    ) -> impl IntoElement {
        let press = cx.listener(move |this, _, _, cx| this.change_inputs(|i| set(i, true), cx));
        let release = cx.listener(move |this, _, _, cx| this.change_inputs(|i| set(i, false), cx));
        let release_outside = cx.listener(move |this, _, _, cx| this.change_inputs(|i| set(i, false), cx));
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .px_3()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(if pressed { BUTTON_PRESSED } else { BUTTON }))
            .cursor_pointer()
            .child(label)
            .on_mouse_down(MouseButton::Left, press)
            .on_mouse_up(MouseButton::Left, release)
            .on_mouse_up_out(MouseButton::Left, release_outside)
    }

    /// Переключатель: остаётся в положении после щелчка.
    fn switch(
        &self,
        id: &'static str,
        label: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        toggle: fn(&mut Inputs),
    ) -> impl IntoElement {
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .child(
                div()
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(if on { ACCENT } else { MUTED }))
                    .bg(rgb(if on { ACCENT } else { BACKGROUND }))
                    .text_color(rgb(BACKGROUND))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if on { "✓" } else { "" }),
            )
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.change_inputs(toggle, cx)))
    }
}

fn set_bit(value: &mut u8, bit: u8, on: bool) {
    if on {
        *value |= 1 << bit;
    } else {
        *value &= !(1 << bit);
    }
}

fn heading(title: &'static str) -> impl IntoElement {
    div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(TEXT)).child(title)
}

fn section(title: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(MUTED)).child(title))
}

fn lamp(on: bool, color: u32, diameter: f32) -> impl IntoElement {
    div()
        .size(px(diameter))
        .rounded_full()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(if on { color } else { LED_OFF }))
}

fn labeled_lamp(name: &'static str, label: &'static str, on: bool) -> impl IntoElement {
    div()
        .debug_selector(|| name.to_string())
        .flex()
        .items_center()
        .gap_2()
        .child(lamp(on, LAMP_ON, 18.0))
        .child(label)
}

/// Кнопка панели управления; обработчик вешается только на включённую кнопку.
fn push_button(
    name: String,
    label: &str,
    enabled: bool,
    selected: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let background = if selected { ACCENT } else { BUTTON };
    let color = rgb(if selected { BACKGROUND } else if enabled { TEXT } else { DIM });
    let mut button = div()
        .id(SharedString::from(name.clone()))
        .debug_selector(move || name)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(background))
        .text_color(color)
        .child(label.to_string());
    if enabled {
        button = button.cursor_pointer().on_click(on_click);
    }
    button
}

// ---------- память ----------

impl DebugWindow {
    fn memory_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut header = div().flex().gap_4().px_3().font_family("Menlo").text_color(rgb(MUTED));
        header = header.child(div().w(px(40.0)).child("адрес"));
        for column in 0..WORDS_PER_ROW {
            header = header.child(div().w(px(76.0)).child(format!("+{column}")));
        }

        div()
            .debug_selector(|| "memory".to_string())
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(heading("RAM: 1024 слова, адреса в словах")),
            )
            .child(div().py_1().child(header))
            .child(
                uniform_list(
                    "ram",
                    ROWS,
                    cx.processor(|this, range: std::ops::Range<usize>, _window, _cx| {
                        range.map(|row| memory_row(row, &this.snapshot.ram)).collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
    }
}

/// Текст строки просмотра памяти: адрес первого слова и слова строки.
pub fn memory_row_text(row: usize, ram: &[u32]) -> (String, Vec<String>) {
    let words = ram[row * WORDS_PER_ROW..(row + 1) * WORDS_PER_ROW]
        .iter()
        .map(|word| format!("{word:08X}"))
        .collect();
    (format!("{:03X}", row * WORDS_PER_ROW), words)
}

fn memory_row(row: usize, ram: &[u32]) -> gpui::Div {
    let (address, words) = memory_row_text(row, ram);
    let mut line = div()
        .debug_selector(move || format!("ram-row-{row}"))
        .flex()
        .gap_4()
        .px_3()
        .font_family("Menlo");
    line = line.child(div().w(px(40.0)).text_color(rgb(MUTED)).child(address));
    for (index, word) in words.into_iter().enumerate() {
        let color = if ram[row * WORDS_PER_ROW + index] == 0 { DIM } else { TEXT };
        line = line.child(div().w(px(76.0)).text_color(rgb(color)).child(word));
    }
    line
}
