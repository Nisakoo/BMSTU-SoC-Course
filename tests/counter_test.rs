use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;

// Обёртки с фиксированной шириной портов лежат в tests/rtl/
#[verilog(src = "tests/rtl/counter4.v", name = "counter4")]
struct Counter4;

#[verilog(src = "tests/rtl/counter8.v", name = "counter8")]
struct Counter8;

// Один такт: clk 0 -> 1
macro_rules! tick {
    ($c:expr) => {{
        $c.clk = 0;
        $c.eval();
        $c.clk = 1;
        $c.eval();
    }};
}

// Создаёт runtime и модель счётчика и делает начальный сброс
macro_rules! new_counter {
    ($runtime:ident, $c:ident, $ty:ty, $wrapper:expr) => {
        let $runtime = VerilatorRuntime::new(
            "build".as_ref(),
            &["rtl/counter.v".as_ref(), $wrapper.as_ref()],
            &[],
            [],
            VerilatorRuntimeOptions::default(),
        )?;
        let mut $c = $runtime.create_model_simple::<$ty>()?;
        $c.reset = 1;
        $c.load = 0;
        $c.load_value = 0;
        tick!($c);
        $c.reset = 0;
    };
}

// Одни и те же проверки для счётчиков разной ширины
macro_rules! counter_tests {
    ($name:ident, $ty:ty, $wrapper:expr, $max:expr) => {
        mod $name {
            use super::*;

            #[test]
            #[snafu::report]
            fn reset_gives_zero() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                assert_eq!(c.count, 0, "после начального сброса");
                for _ in 0..5 {
                    tick!(c);
                }
                assert_eq!(c.count, 5);
                c.reset = 1;
                tick!(c);
                assert_eq!(c.count, 0, "после сброса посреди счёта");
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn counts_up_by_one() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                for i in 1..=10 {
                    tick!(c);
                    assert_eq!(c.count, i, "такт {i}");
                }
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn full_cycle_returns_to_start() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                let modulus = $max as u32 + 1;
                for i in 1..=modulus {
                    tick!(c);
                    assert_eq!(c.count as u32, i % modulus, "такт {i}");
                }
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn wraps_from_max_to_zero() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                c.load = 1;
                c.load_value = $max;
                tick!(c);
                assert_eq!(c.count, $max, "загружен максимум");
                c.load = 0;
                tick!(c);
                assert_eq!(c.count, 0, "переполнение");
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn load_sets_value_and_counting_continues() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                c.load = 1;
                c.load_value = 5;
                tick!(c);
                assert_eq!(c.count, 5, "загрузка");
                c.load = 0;
                tick!(c);
                assert_eq!(c.count, 6, "счёт продолжается с загруженного значения");
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn load_value_ignored_without_load() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                c.load = 0;
                c.load_value = 9;
                tick!(c);
                assert_eq!(c.count, 1, "load = 0: идёт обычный счёт");
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn reset_beats_load() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                for _ in 0..3 {
                    tick!(c);
                }
                c.reset = 1;
                c.load = 1;
                c.load_value = 7;
                tick!(c);
                assert_eq!(c.count, 0, "сброс приоритетнее загрузки");
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn reset_at_max_value() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                c.load = 1;
                c.load_value = $max;
                tick!(c);
                c.load = 0;
                c.reset = 1;
                tick!(c);
                assert_eq!(c.count, 0);
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn changes_only_on_rising_edge() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                c.clk = 0;
                c.eval();
                assert_eq!(c.count, 0, "clk = 0");
                c.clk = 1;
                c.eval();
                assert_eq!(c.count, 1, "фронт 0 -> 1");
                c.eval();
                assert_eq!(c.count, 1, "clk остался 1");
                c.clk = 0;
                c.eval();
                assert_eq!(c.count, 1, "спад 1 -> 0");
                Ok(())
            }

            #[test]
            #[snafu::report]
            fn reset_is_synchronous() -> Result<(), VerilatorError> {
                new_counter!(runtime, c, $ty, $wrapper);
                for _ in 0..3 {
                    tick!(c);
                }
                c.reset = 1;
                c.eval();
                assert_eq!(c.count, 3, "reset без фронта clk не сбрасывает");
                tick!(c);
                assert_eq!(c.count, 0, "сброс по фронту clk");
                Ok(())
            }
        }
    };
}

counter_tests!(counter4, Counter4, "tests/rtl/counter4.v", 15);
counter_tests!(counter8, Counter8, "tests/rtl/counter8.v", 255);
