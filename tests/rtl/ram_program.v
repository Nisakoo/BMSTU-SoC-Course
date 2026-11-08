// Обёртка для тестов: RAM, загруженная из tests/data/ram_test.hex
module ram_program(
    input         clk,
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] data
);
    ram #(.INIT_FILE("tests/data/ram_test.hex")) u_ram (
        .clk (clk),
        .addr(addr),
        .we  (we),
        .wd  (wd),
        .data(data)
    );
endmodule
