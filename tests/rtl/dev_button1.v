// Обёртка для тестов: кнопка с одним входом по адресу 0x020
module dev_button1(
    input         clk,
    input         reset,
    input  [9:0]  addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    input         btn
);
    dev_button #(.BASE(10'h020), .WIDTH(1)) u_button (
        .clk(clk), .reset(reset), .addr(addr), .we(we), .wd(wd), .rd(rd), .btn(btn)
    );
endmodule
