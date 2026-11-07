use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use soc::Main;

#[test]
fn forwards_u32max_correctly() -> Result<(), VerilatorError> {
    let runtime = VerilatorRuntime::new(
        "build".as_ref(),
        &["src/main.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )?;

    let mut main = runtime.create_model_simple::<Main>()?;

    main.medium_input = u32::MAX;
    println!("{}", main.medium_output);
    assert_eq!(main.medium_output, 0);
    main.eval();
    println!("{}", main.medium_output);
    assert_eq!(main.medium_output, u32::MAX);

    Ok(())
}
