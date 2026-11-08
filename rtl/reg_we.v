module reg_we #(
    parameter WIDTH = 32
)(
    input                  clk,
    input                  reset,
    input                  we,
    input      [WIDTH-1:0] d,
    output reg [WIDTH-1:0] q
);
    always @(posedge clk) begin
        if (reset)
            q <= {WIDTH{1'b0}};
        else if (we)
            q <= d;
    end
endmodule
