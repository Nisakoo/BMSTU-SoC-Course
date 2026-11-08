// Обёртка для тестов: один светодиод по адресу 0x030
module dev_led1(
    input         clk,
    input         reset,
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    output        led
);
    dev_led #(.BASE(10'h030), .WIDTH(1)) u_led (
        .clk(clk), .reset(reset), .addr(addr), .we(we), .wd(wd), .rd(rd), .led(led)
    );
endmodule
