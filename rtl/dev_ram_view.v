module dev_ram_view(
    /* verilator lint_off UNUSEDSIGNAL */
    input         clk,
    input         reset,
    // шина устройства
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    /* verilator lint_on UNUSEDSIGNAL */
    output [31:0] rd,                  // читаемых регистров нет
    // порт просмотра RAM
    output [9:0]  ram_raddr,
    input  [31:0] ram_rdata,
    // выводы для Rust
    input  [9:0]  view_addr,           // адрес слова RAM, ставит Rust
    output [31:0] view_data            // слово RAM
);
    // мост между Rust и портом чтения RAM
    assign ram_raddr = view_addr;
    assign view_data = ram_rdata;
    assign rd        = 32'd0;
endmodule
