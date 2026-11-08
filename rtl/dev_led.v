module dev_led #(
    parameter [9:0] BASE  = 10'h030,   // адрес регистра в MMIO
    parameter       WIDTH = 1          // число светодиодов (от 1 до 32), по биту на каждый
)(
    input               clk,
    input               reset,
    // шина устройства
    input  [9:0]        addr,
    input               we,
    /* verilator lint_off UNUSEDSIGNAL */
    input  [31:0]       wd,            // используются младшие WIDTH бит
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0]       rd,
    // внешние выводы
    output [WIDTH-1:0]  led
);
    reg [WIDTH-1:0] led_r;

    always @(posedge clk) begin
        if (reset)
            led_r <= {WIDTH{1'b0}};
        else if (we && addr == BASE)
            led_r <= wd[WIDTH-1:0];
    end

    // записанное значение читается обратно, остальные биты равны 0
    reg [31:0] value;
    always @* begin
        value = 32'd0;
        value[WIDTH-1:0] = led_r;
    end

    assign led = led_r;
    assign rd  = (addr == BASE) ? value : 32'd0;
endmodule
