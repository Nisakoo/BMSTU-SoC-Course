use marlin::verilog::prelude::*;

#[verilog(src = "rtl/main.v", name = "main")]
pub struct Main;

#[verilog(src = "rtl/alu.v", name = "alu")]
pub struct ALU;
