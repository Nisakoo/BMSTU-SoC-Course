//! Симуляция SoC в отдельном потоке.
//!
//! Модель Verilator нельзя передавать между потоками, поэтому её создаёт и держит
//! поток симуляции. Остальной код общается с ним командами (`Sim::step`, `Sim::run`, ...)
//! и читает снимки состояния (`Sim::snapshot`).
//!
//! Работает из корня проекта: пути к `rtl/`, `sim/` и `build/` относительные.

use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

/// Размер RAM в словах.
pub const RAM_WORDS: usize = 1024;

/// Не чаще, чем раз в это время, обновляется снимок во время непрерывного запуска.
const PUBLISH_PERIOD: Duration = Duration::from_millis(33);
/// Сколько тактов за раз выполняется между проверками команд.
const BATCH: u64 = 500;

/// Состояние входов SoC.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Inputs {
    /// кнопка реакции
    pub btn_react: bool,
    /// кнопки пульта: бит 0 — открыть, бит 1 — закрыть
    pub remote: u8,
    /// концевые выключатели: бит 0 — открыто, бит 1 — закрыто
    pub limit: u8,
    /// ИК-датчик препятствия
    pub ir_sensor: bool,
}

/// Скорость непрерывного запуска.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    /// столько тактов в секунду
    PerSecond(u64),
    /// как можно быстрее
    Max,
}

/// Состояние SoC в момент публикации.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Растёт при каждой публикации: по нему видно, что снимок изменился.
    pub version: u64,
    /// Выполнено тактов после сброса.
    pub ticks: u64,
    /// Идёт непрерывный запуск.
    pub running: bool,
    /// Процессор остановлен командой `HALT`.
    pub halted: bool,
    pub led: u8,
    pub gate_open: bool,
    pub gate_close: bool,
    /// Все слова RAM.
    pub ram: Vec<u32>,
}

impl Snapshot {
    fn initial() -> Self {
        Snapshot {
            version: 0,
            ticks: 0,
            running: false,
            halted: false,
            led: 0,
            gate_open: false,
            gate_close: false,
            ram: vec![0; RAM_WORDS],
        }
    }
}

enum Command {
    Step,
    Run,
    Pause,
    SetSpeed(Speed),
    SetInputs(Inputs),
    Stop,
}

pub struct Sim {
    commands: Sender<Command>,
    shared: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}

impl Sim {
    /// Загружает образы ROM и RAM (hex-файлы ассемблера) и запускает поток симуляции.
    /// Возвращается, когда модель готова; первая сборка модели Verilator занимает несколько секунд.
    pub fn start(rom: &Path, ram: &Path) -> Result<Sim, String> {
        // параметры модуля из Rust задать нельзя: обёртка читает файлы с фиксированными именами
        fs::create_dir_all("build").map_err(|e| format!("build: {e}"))?;
        fs::copy(rom, "build/sim_program.hex").map_err(|e| format!("{}: {e}", rom.display()))?;
        fs::copy(ram, "build/sim_data.hex").map_err(|e| format!("{}: {e}", ram.display()))?;

        let (commands, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(Snapshot::initial()));
        let worker_shared = Arc::clone(&shared);
        let thread = std::thread::spawn(move || worker(receiver, worker_shared, ready_tx));

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Sim { commands, shared, thread: Some(thread) }),
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(message)
            }
            Err(_) => {
                let _ = thread.join();
                Err("поток симуляции завершился при запуске".to_string())
            }
        }
    }

    /// Один такт. Если процессор остановлен, ничего не делает.
    pub fn step(&self) {
        let _ = self.commands.send(Command::Step);
    }

    /// Непрерывный запуск со скоростью, заданной `set_speed`. Останавливается по `HALT`.
    pub fn run(&self) {
        let _ = self.commands.send(Command::Run);
    }

    pub fn pause(&self) {
        let _ = self.commands.send(Command::Pause);
    }

    pub fn set_speed(&self, speed: Speed) {
        let _ = self.commands.send(Command::SetSpeed(speed));
    }

    pub fn set_inputs(&self, inputs: Inputs) {
        let _ = self.commands.send(Command::SetInputs(inputs));
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.shared).clone()
    }

    /// Номер последнего снимка, без копирования памяти.
    pub fn version(&self) -> u64 {
        lock(&self.shared).version
    }
}

impl Drop for Sim {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn lock(shared: &Mutex<Snapshot>) -> MutexGuard<'_, Snapshot> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
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

/// Создаёт модель; ошибка сборки или загрузки превращается в текст.
macro_rules! try_or_report {
    ($result:expr, $ready:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => {
                let _ = $ready.send(Err(error.to_string()));
                return;
            }
        }
    };
}

fn worker(commands: Receiver<Command>, shared: Arc<Mutex<Snapshot>>, ready: Sender<Result<(), String>>) {
    let files: Vec<&Path> = SOC_FILES.iter().map(|f| Path::new(*f)).collect();
    let runtime = try_or_report!(
        VerilatorRuntime::new("build".as_ref(), &files, &[], [], VerilatorRuntimeOptions::default()),
        ready
    );
    let mut m = try_or_report!(runtime.create_model_simple::<SocSimTop>(), ready);

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
    let _ = ready.send(Ok(()));

    let mut ticks = 0u64;
    let mut running = false;
    let mut halted = false;
    let mut speed = Speed::Max;
    let mut credit = 0.0f64;
    let mut last_pace = Instant::now();
    let mut version = 0u64;
    let mut last_publish: Instant;

    // Копирует состояние в общий снимок. Чтение RAM комбинационное и состояние не меняет.
    macro_rules! publish {
        () => {{
            let mut ram = Vec::with_capacity(RAM_WORDS);
            for addr in 0..RAM_WORDS as u16 {
                m.ram_view_addr = addr;
                m.eval();
                ram.push(m.ram_view_data);
            }
            version += 1;
            *lock(&shared) = Snapshot {
                version,
                ticks,
                running,
                halted,
                led: m.led,
                gate_open: m.gate_open != 0,
                gate_close: m.gate_close != 0,
                ram,
            };
            last_publish = Instant::now();
        }};
    }

    publish!();
    'main: loop {
        let wait = match (running && !halted, speed) {
            (true, Speed::Max) => Duration::ZERO,
            (true, Speed::PerSecond(_)) => Duration::from_millis(1),
            (false, _) => Duration::from_millis(50),
        };
        let mut pending = Vec::new();
        match commands.recv_timeout(wait) {
            Ok(command) => {
                pending.push(command);
                while let Ok(command) = commands.try_recv() {
                    pending.push(command);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let mut force = !pending.is_empty();
        for command in pending {
            match command {
                Command::Step => {
                    if !halted {
                        tick!(m);
                        ticks += 1;
                        halted = m.halt != 0;
                        running = running && !halted;
                    }
                }
                Command::Run => {
                    if !halted {
                        running = true;
                        credit = 0.0;
                        last_pace = Instant::now();
                    }
                }
                Command::Pause => running = false,
                Command::SetSpeed(new_speed) => {
                    speed = new_speed;
                    credit = 0.0;
                    last_pace = Instant::now();
                }
                Command::SetInputs(inputs) => {
                    m.btn_react = u8::from(inputs.btn_react);
                    m.remote = inputs.remote & 0b11;
                    m.limit = inputs.limit & 0b11;
                    m.ir_sensor = u8::from(inputs.ir_sensor);
                    m.eval();
                }
                Command::Stop => break 'main,
            }
        }

        let mut changed = force;
        if running && !halted {
            let count = match speed {
                Speed::Max => BATCH,
                Speed::PerSecond(rate) => {
                    let now = Instant::now();
                    credit += now.duration_since(last_pace).as_secs_f64() * rate as f64;
                    last_pace = now;
                    let count = (credit as u64).min(BATCH * 4);
                    credit = (credit - count as f64).min((BATCH * 4) as f64);
                    count
                }
            };
            for _ in 0..count {
                tick!(m);
                ticks += 1;
                changed = true;
                if m.halt != 0 {
                    halted = true;
                    running = false;
                    force = true;
                    break;
                }
            }
        }

        if changed && (force || last_publish.elapsed() >= PUBLISH_PERIOD) {
            publish!();
        }
    }
}
