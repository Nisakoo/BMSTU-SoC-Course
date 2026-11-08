module regfile(
    input           clk,
    input           reset,
    input   [3:0]   ra1,
    input   [3:0]   ra2,
    input   [3:0]   wa,
    input   [31:0]  wd,
    input           we,
    output  [31:0]  rd1,
    output  [31:0]  rd2
);
    reg [31:0] regs [0:15];
    integer i;

    always @(posedge clk) begin
        if (reset) begin
            for (i = 0; i < 16; i = i + 1)
                regs[i] <= 32'd0;
        end else if (we && wa != 4'd0) begin // Регистр R0 всегда равен нулю
            regs[wa] <= wd;
        end
    end

    assign rd1 = (ra1 == 4'd0) ? 32'd0 : regs[ra1];
    assign rd2 = (ra2 == 4'd0) ? 32'd0 : regs[ra2];
endmodule
