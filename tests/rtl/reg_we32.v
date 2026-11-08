// Обёртка для тестов: reg_we с фиксированной шириной 32
module reg_we32(
    input         clk,
    input         reset,
    input         we,
    input  [31:0] d,
    output [31:0] q
);
    reg_we #(.WIDTH(32)) u_reg (
        .clk  (clk),
        .reset(reset),
        .we   (we),
        .d    (d),
        .q    (q)
    );
endmodule
