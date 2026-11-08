// Обёртка для тестов: видеопамять и дисплей, соединённые так же, как в soc_top.
// Порт процессора видеопамяти выведен наружу, порт дисплея идёт через устройство-дисплей.
module display_vram(
    input         clk,
    // порт процессора видеопамяти
    input  [11:0] addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    // шина устройства дисплея
    input  [9:0]  bus_addr,
    input         bus_we,
    input  [31:0] bus_wd,
    output [31:0] bus_rd,
    // выводы для Rust
    input  [11:0] frame_addr,
    output [31:0] frame_data
);
    wire [11:0] raddr;
    wire [31:0] rdata;

    vram u_vram (
        .clk  (clk),
        .addr (addr),
        .we   (we),
        .wd   (wd),
        .rd   (rd),
        .raddr(raddr),
        .rdata(rdata)
    );

    dev_display u_display (
        .clk       (clk),
        .reset     (1'b0),
        .addr      (bus_addr),
        .we        (bus_we),
        .wd        (bus_wd),
        .rd        (bus_rd),
        .vram_raddr(raddr),
        .vram_rdata(rdata),
        .frame_addr(frame_addr),
        .frame_data(frame_data)
    );
endmodule
