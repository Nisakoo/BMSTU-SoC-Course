module dev_display(
    /* verilator lint_off UNUSEDSIGNAL */
    input         clk,
    input         reset,
    // шина устройства
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0] rd,                  // читаемых регистров нет
    // порт дисплея видеопамяти
    output [11:0] vram_raddr,
    input  [31:0] vram_rdata,
    // выводы для Rust
    input  [11:0] frame_addr,          // адрес слова кадра, ставит Rust
    output [31:0] frame_data           // слово кадра (4 пикселя)
);
    // дисплей — мост между Rust и портом чтения видеопамяти
    assign vram_raddr = frame_addr;
    assign frame_data = vram_rdata;
    assign rd         = 32'd0;
endmodule
