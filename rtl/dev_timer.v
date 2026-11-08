module dev_timer #(
    parameter [9:0]  BASE = 10'h000,   // адрес регистра таймера в MMIO
    parameter [15:0] DIV  = 16'd50     // тактов на одну микросекунду
)(
    input         clk,
    input         reset,
    // шина устройства
    input  [9:0]  addr,
    /* verilator lint_off UNUSEDSIGNAL */
    input         we,                  // таймер только читается, запись игнорируется
    input  [31:0] wd,
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0] rd,
    // связь с другими устройствами
    output [31:0] time_us              // микросекунды с момента сброса
);
    reg [15:0] div;
    reg [31:0] time_r;

    always @(posedge clk) begin
        if (reset) begin
            div    <= 16'd0;
            time_r <= 32'd0;
        end else if (div == DIV - 16'd1) begin
            div    <= 16'd0;
            time_r <= time_r + 32'd1;
        end else begin
            div <= div + 16'd1;
        end
    end

    assign time_us = time_r;
    assign rd = (addr == BASE) ? time_r : 32'd0;
endmodule
