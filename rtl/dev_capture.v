module dev_capture #(
    parameter [9:0] BASE = 10'h010     // адрес регистра arm в MMIO
)(
    input         clk,
    input         reset,
    // шина устройства
    input  [9:0]  addr,
    input         we,
    /* verilator lint_off UNUSEDSIGNAL */
    input  [31:0] wd,                  // взводит любая запись, значение не важно
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0] rd,                  // читаемых регистров нет
    // внешний вывод и связь с другими устройствами
    input         btn,                 // кнопка (асинхронный сигнал)
    input  [31:0] time_us,             // значение таймера
    // связь с ядром (HIGH_RES_TICK)
    output [31:0] capt,                // захваченное значение
    output        nocap                // 1: захвата не было
);
    // синхронизатор: два триггера и предыдущее значение для поиска фронта
    reg sync0, sync1, prev;
    reg armed, captured;
    reg [31:0] capt_r;

    wire rising = sync1 & ~prev;
    wire arm_wr = we & (addr == BASE);

    always @(posedge clk) begin
        if (reset) begin
            sync0    <= 1'b0;
            sync1    <= 1'b0;
            prev     <= 1'b0;
            armed    <= 1'b0;
            captured <= 1'b0;
            capt_r   <= 32'd0;
        end else begin
            sync0 <= btn;
            sync1 <= sync0;
            prev  <= sync1;

            if (arm_wr) begin
                // взвод: прежний результат сбрасывается, фронт в этом же такте игнорируется
                armed    <= 1'b1;
                captured <= 1'b0;
                capt_r   <= 32'd0;
            end else if (armed & rising) begin
                capt_r   <= time_us;
                captured <= 1'b1;
                armed    <= 1'b0;
            end
        end
    end

    assign capt  = capt_r;
    assign nocap = ~captured;
    assign rd    = 32'd0;
endmodule
