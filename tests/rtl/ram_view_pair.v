// Обёртка для тестов: RAM и устройство просмотра, соединённые так же, как в soc_top.
// Порт процессора RAM выведен наружу, порт просмотра идёт через устройство.
module ram_view_pair(
    input         clk,
    // порт процессора RAM
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] data,
    // шина устройства просмотра
    input  [9:0]  bus_addr,
    input         bus_we,
    input  [31:0] bus_wd,
    output [31:0] bus_rd,
    // выводы для Rust
    input  [9:0]  view_addr,
    output [31:0] view_data
);
    wire [9:0]  raddr;
    wire [31:0] rdata;

    ram u_ram (
        .clk  (clk),
        .addr (addr),
        .we   (we),
        .wd   (wd),
        .data (data),
        .raddr(raddr),
        .rdata(rdata)
    );

    dev_ram_view u_view (
        .clk      (clk),
        .reset    (1'b0),
        .addr     (bus_addr),
        .we       (bus_we),
        .wd       (bus_wd),
        .rd       (bus_rd),
        .ram_raddr(raddr),
        .ram_rdata(rdata),
        .view_addr(view_addr),
        .view_data(view_data)
    );
endmodule
