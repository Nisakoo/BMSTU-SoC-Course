module dev_button #(
    parameter [9:0] BASE  = 10'h020,   // адрес регистра в MMIO
    parameter       WIDTH = 1          // число входов (от 1 до 32), по биту на каждый
)(
    input              clk,
    input              reset,
    // шина устройства
    input  [9:0]       addr,
    /* verilator lint_off UNUSEDSIGNAL */
    input              we,             // регистр только читается, запись игнорируется
    input  [31:0]      wd,
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0]      rd,
    // внешние выводы
    input  [WIDTH-1:0] btn             // кнопки, концевые выключатели, датчики
);
    // синхронизатор: два триггера на каждый вход
    reg [WIDTH-1:0] sync0, sync1;

    always @(posedge clk) begin
        if (reset) begin
            sync0 <= {WIDTH{1'b0}};
            sync1 <= {WIDTH{1'b0}};
        end else begin
            sync0 <= btn;
            sync1 <= sync0;
        end
    end

    // состояние входов в младших битах, остальные биты равны 0
    reg [31:0] state;
    always @* begin
        state = 32'd0;
        state[WIDTH-1:0] = sync1;
    end

    assign rd = (addr == BASE) ? state : 32'd0;
endmodule
