module dev_gate #(
    parameter [9:0] BASE = 10'h040     // адрес регистра в MMIO
)(
    input         clk,
    input         reset,
    // шина устройства
    input  [9:0]  addr,
    input         we,
    /* verilator lint_off UNUSEDSIGNAL */
    input  [31:0] wd,                  // используются биты 0 и 1
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0] rd,
    // внешние выводы: привод ворот
    output        gate_open,           // мотор на открытие
    output        gate_close           // мотор на закрытие
);
    // бит 0 — открыть, бит 1 — закрыть
    reg open_r, close_r;

    always @(posedge clk) begin
        if (reset) begin
            open_r  <= 1'b0;
            close_r <= 1'b0;
        end else if (we && addr == BASE) begin
            open_r  <= wd[0];
            close_r <= wd[1];
        end
    end

    // оба бита сразу: мотор останавливается (защита)
    assign gate_open  = open_r  & ~close_r;
    assign gate_close = close_r & ~open_r;

    // читается записанное значение
    assign rd = (addr == BASE) ? {30'd0, close_r, open_r} : 32'd0;
endmodule
