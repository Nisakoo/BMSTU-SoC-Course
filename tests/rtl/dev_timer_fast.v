// Обёртка для тестов: таймер с малым делителем (3 такта на «микросекунду») и регистром по адресу 0x020
module dev_timer_fast(
    input         clk,
    input         reset,
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    output [31:0] time_us
);
    dev_timer #(.BASE(10'h020), .DIV(16'd3)) u_timer (
        .clk    (clk),
        .reset  (reset),
        .addr   (addr),
        .we     (we),
        .wd     (wd),
        .rd     (rd),
        .time_us(time_us)
    );
endmodule
