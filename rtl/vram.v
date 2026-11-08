module vram(
    input         clk,
    // порт процессора: чтение и запись
    input  [11:0] addr,
    input         we,
    input  [31:0] wd,
    output [31:0] rd,
    // порт дисплея: только чтение
    input  [11:0] raddr,
    output [31:0] rdata
);
    reg [31:0] mem [0:4095];
    integer i;

    initial begin
        for (i = 0; i < 4096; i = i + 1)
            mem[i] = 32'd0;
    end

    always @(posedge clk) begin
        if (we)
            mem[addr] <= wd;
    end

    assign rd    = mem[addr];
    assign rdata = mem[raddr];
endmodule
