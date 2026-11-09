// Обёртка для тестов ассемблера: soc_top с программой и данными из файлов,
// которые пишет ассемблер (build/asm_program.hex и build/asm_data.hex).
module soc_asm_top(
    input         clk,
    input         reset,
    input         btn_react,
    input  [1:0]  remote,
    input  [1:0]  limit,
    input         ir_sensor,
    output [7:0]  led,
    output        gate_open,
    output        gate_close,
    input  [11:0] frame_addr,
    output [31:0] frame_data,
    input  [9:0]  ram_view_addr,
    output [31:0] ram_view_data,
    output        halt
);
    soc_top #(.PROGRAM_FILE("build/asm_program.hex"), .DATA_FILE("build/asm_data.hex")) u_soc (
        .clk       (clk),
        .reset     (reset),
        .btn_react (btn_react),
        .remote    (remote),
        .limit     (limit),
        .ir_sensor (ir_sensor),
        .led       (led),
        .gate_open (gate_open),
        .gate_close(gate_close),
        .frame_addr(frame_addr),
        .frame_data(frame_data),
        .ram_view_addr(ram_view_addr),
        .ram_view_data(ram_view_data),
        .halt      (halt)
    );
endmodule
