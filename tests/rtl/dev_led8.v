// Обёртка для тестов: восемь светодиодов в одном регистре по адресу 0x031
module dev_led8(
    input         clk,
    input         reset,
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    output [7:0]  led
);
    dev_led #(.BASE(10'h031), .WIDTH(8)) u_led (
        .clk(clk), .reset(reset), .addr(addr), .we(we), .wd(wd), .rd(rd), .led(led)
    );
endmodule
