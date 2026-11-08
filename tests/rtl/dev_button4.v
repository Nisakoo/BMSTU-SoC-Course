// Обёртка для тестов: четыре входа в одном регистре по адресу 0x025
module dev_button4(
    input         clk,
    input         reset,
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    input  [3:0]  btn
);
    dev_button #(.BASE(10'h025), .WIDTH(4)) u_button (
        .clk(clk), .reset(reset), .addr(addr), .we(we), .wd(wd), .rd(rd), .btn(btn)
    );
endmodule
