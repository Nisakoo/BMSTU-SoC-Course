module rom #(
    parameter INIT_FILE = ""
)(
    input   [9:0]   addr,
    output  [31:0]  data
);
    reg [31:0] mem [0:1023];
    integer i;

    initial begin
        for (i = 0; i < 1024; i = i + 1)
            mem[i] = 32'd0;
        if (INIT_FILE != "")
            $readmemh(INIT_FILE, mem);
    end

    assign data = mem[addr];
endmodule
