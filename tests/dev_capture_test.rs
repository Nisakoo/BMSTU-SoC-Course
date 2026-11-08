use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Захват с параметрами по умолчанию: регистр arm по адресу 0x010
#[verilog(src = "rtl/dev_capture.v", name = "dev_capture")]
struct DevCapture;

const ARM_ADDR: u16 = 0x010;

// Кнопка проходит синхронизатор из двух триггеров, потом ищется фронт:
// фронт сохраняется на третьем такте после изменения кнопки.
const LATENCY: u32 = 3;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($c:expr) => {{
        $c.clk = 0;
        $c.eval();
        $c.clk = 1;
        $c.eval();
    }};
}

// Такт с новым значением времени на входе: t увеличивается на 1
macro_rules! step {
    ($c:expr, $t:expr) => {{
        $t += 1;
        $c.time_us = $t;
        tick!($c);
    }};
}

// Запись в регистр arm на одном такте
macro_rules! arm {
    ($c:expr, $t:expr) => {{
        $c.addr = ARM_ADDR;
        $c.we = 1;
        $c.wd = 0xFFFF_FFFF;
        step!($c, $t);
        $c.we = 0;
        $c.wd = 0;
    }};
}

// Создаёт модель, делает сброс; $t — счётчик времени
macro_rules! new_capture {
    ($runtime:ident, $c:ident, $t:ident) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/dev_capture.v".as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $c = $runtime.create_model_simple::<DevCapture>()?;
        #[allow(unused_mut)]
        let mut $t: u32 = 1000;
        $c.reset = 1;
        $c.we = 0;
        $c.wd = 0;
        $c.addr = 0;
        $c.btn = 0;
        $c.time_us = $t;
        tick!($c);
        $c.reset = 0;
    };
}

#[test]
#[snafu::report]
fn nothing_is_captured_after_reset() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    assert_eq!(c.nocap, 1, "захвата нет");
    assert_eq!(c.capt, 0, "регистр захвата пуст");
    Ok(())
}

#[test]
#[snafu::report]
fn press_without_arm_is_ignored() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    c.btn = 1;
    for _ in 0..10 {
        step!(c, t);
    }
    c.btn = 0;
    for _ in 0..5 {
        step!(c, t);
    }
    c.btn = 1;
    for _ in 0..10 {
        step!(c, t);
    }
    assert_eq!(c.nocap, 1);
    assert_eq!(c.capt, 0);
    Ok(())
}

#[test]
#[snafu::report]
fn armed_press_captures_the_time_of_the_third_clock() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    assert_eq!(c.nocap, 1, "после взвода захвата ещё нет");
    c.btn = 1;
    for i in 1..LATENCY {
        step!(c, t);
        assert_eq!(c.nocap, 1, "такт {i}: фронт ещё не дошёл");
    }
    step!(c, t);
    assert_eq!(c.nocap, 0, "захват был");
    assert_eq!(c.capt, t, "захвачено время на третьем такте после нажатия");
    Ok(())
}

#[test]
#[snafu::report]
fn captured_value_is_held_and_later_presses_are_ignored() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t);
    }
    let captured = c.capt;
    assert_eq!(c.nocap, 0);
    // много отпусканий и нажатий, время идёт
    for round in 0..5 {
        c.btn = 0;
        for _ in 0..4 {
            step!(c, t);
        }
        c.btn = 1;
        for _ in 0..4 {
            step!(c, t);
        }
        assert_eq!(c.capt, captured, "раунд {round}");
        assert_eq!(c.nocap, 0, "раунд {round}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn rearm_clears_the_result_and_allows_a_new_capture() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t);
    }
    let first = c.capt;
    assert_eq!(c.nocap, 0);

    c.btn = 0;
    for _ in 0..5 {
        step!(c, t);
    }
    arm!(c, t);
    assert_eq!(c.nocap, 1, "после повторного взвода захвата снова нет");
    assert_eq!(c.capt, 0, "прежнее значение сброшено");

    for _ in 0..5 {
        step!(c, t); // время идёт, кнопка отпущена
    }
    assert_eq!(c.nocap, 1, "без нового нажатия захвата нет");
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t);
    }
    assert_eq!(c.nocap, 0);
    assert_eq!(c.capt, t, "новое время");
    assert_ne!(c.capt, first);
    Ok(())
}

#[test]
#[snafu::report]
fn button_already_held_at_arm_does_not_capture() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    c.btn = 1; // кнопка зажата заранее
    for _ in 0..10 {
        step!(c, t);
    }
    arm!(c, t);
    for _ in 0..10 {
        step!(c, t);
    }
    assert_eq!(c.nocap, 1, "нет нового фронта: захват не срабатывает");
    // отпустить и нажать заново
    c.btn = 0;
    for _ in 0..5 {
        step!(c, t);
    }
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t);
    }
    assert_eq!(c.nocap, 0, "новое нажатие захвачено");
    assert_eq!(c.capt, t);
    Ok(())
}

#[test]
#[snafu::report]
fn edge_in_the_same_clock_as_arm_is_ignored() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    c.btn = 1;
    for _ in 0..(LATENCY - 1) {
        step!(c, t); // фронт будет замечен на следующем такте
    }
    arm!(c, t); // взвод ровно на том такте, где был бы захват
    assert_eq!(c.nocap, 1, "взвод приоритетнее фронта");
    for _ in 0..10 {
        step!(c, t); // кнопка по-прежнему зажата, нового фронта нет
    }
    assert_eq!(c.nocap, 1);
    Ok(())
}

#[test]
#[snafu::report]
fn rearm_wins_over_an_edge_in_the_same_clock() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t); // уже взведён
    c.btn = 1;
    for _ in 0..(LATENCY - 1) {
        step!(c, t); // фронт будет замечен на следующем такте
    }
    arm!(c, t); // повторный взвод ровно на этом такте
    assert_eq!(c.nocap, 1, "взвод приоритетнее фронта: захвата нет");
    assert_eq!(c.capt, 0);
    for _ in 0..10 {
        step!(c, t); // кнопка зажата, нового фронта нет
    }
    assert_eq!(c.nocap, 1);
    // захват остался взведённым: новое нажатие ловится
    c.btn = 0;
    for _ in 0..4 {
        step!(c, t);
    }
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t);
    }
    assert_eq!(c.nocap, 0);
    assert_eq!(c.capt, t);
    Ok(())
}

#[test]
#[snafu::report]
fn one_clock_pulse_is_captured() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    c.btn = 1;
    step!(c, t); // кнопка 1 только на один такт
    c.btn = 0;
    for _ in 1..LATENCY {
        step!(c, t);
    }
    assert_eq!(c.nocap, 0, "короткий импульс пойман");
    assert_eq!(c.capt, t);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_clears_the_capture_and_disarms() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t);
    }
    assert_eq!(c.nocap, 0);

    c.reset = 1;
    step!(c, t);
    c.reset = 0;
    assert_eq!(c.nocap, 1, "после сброса захвата нет");
    assert_eq!(c.capt, 0);

    // после сброса захват не взведён: нажатия игнорируются
    c.btn = 0;
    for _ in 0..4 {
        step!(c, t);
    }
    c.btn = 1;
    for _ in 0..10 {
        step!(c, t);
    }
    assert_eq!(c.nocap, 1);
    Ok(())
}

#[test]
#[snafu::report]
fn reset_before_the_edge_disarms() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    c.reset = 1;
    step!(c, t);
    c.reset = 0;
    c.btn = 0;
    step!(c, t);
    c.btn = 1;
    for _ in 0..10 {
        step!(c, t);
    }
    assert_eq!(c.nocap, 1, "взвод сбрасывается общим сбросом");
    Ok(())
}

#[test]
#[snafu::report]
fn only_a_write_to_the_arm_address_arms() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    for addr in 0..1024u16 {
        // чистое состояние перед каждым адресом
        c.reset = 1;
        c.btn = 0;
        step!(c, t);
        c.reset = 0;

        c.addr = addr;
        c.we = 1;
        c.wd = 1;
        step!(c, t);
        c.we = 0;

        c.btn = 1;
        for _ in 0..LATENCY {
            step!(c, t);
        }
        let captured = c.nocap == 0;
        assert_eq!(captured, addr == ARM_ADDR, "запись по адресу {addr:#05x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn no_arm_without_we() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    c.addr = ARM_ADDR;
    c.we = 0;
    c.wd = 0xFFFF_FFFF;
    step!(c, t);
    c.btn = 1;
    for _ in 0..10 {
        step!(c, t);
    }
    assert_eq!(c.nocap, 1, "обращение по адресу arm без we не взводит захват");
    Ok(())
}

#[test]
#[snafu::report]
fn any_written_value_arms() -> Result<(), VerilatorError> {
    for value in [0u32, 1, 0xFFFF_FFFF, 0x8000_0000, 0x1234_5678] {
        new_capture!(runtime, c, t);
        c.addr = ARM_ADDR;
        c.we = 1;
        c.wd = value;
        step!(c, t);
        c.we = 0;
        c.btn = 1;
        for _ in 0..LATENCY {
            step!(c, t);
        }
        assert_eq!(c.nocap, 0, "значение {value:#x}");
    }
    Ok(())
}

#[test]
#[snafu::report]
fn bus_read_is_always_zero() -> Result<(), VerilatorError> {
    new_capture!(runtime, c, t);
    arm!(c, t);
    c.btn = 1;
    for _ in 0..LATENCY {
        step!(c, t); // теперь в регистре захвата ненулевое значение
    }
    assert_ne!(c.capt, 0);
    for we in 0..2u8 {
        c.we = we;
        for addr in 0..1024u16 {
            c.addr = addr;
            c.eval();
            assert_eq!(c.rd, 0, "адрес {addr:#05x}, we = {we}");
        }
    }
    Ok(())
}
