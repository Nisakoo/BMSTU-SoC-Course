use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use soc::ALU;

#[test]
#[snafu::report]
fn alu_add_work_correctly() -> Result<(), VerilatorError> {
    let runtime = VerilatorRuntime::new(
        "build".as_ref(),
        &["rtl/alu.v".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )?;

    let mut alu = runtime.create_model_simple::<ALU>()?;

    alu.a = 3u32;
    alu.b = 5u32;
    alu.op = 0u8;

    assert_eq!(alu.result, 0);

    alu.eval();

    println!("{}", alu.result);
    assert_eq!(alu.result, 8);

    Ok(())
}
