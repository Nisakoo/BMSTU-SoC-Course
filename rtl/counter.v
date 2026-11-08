module counter #(
    parameter WIDTH = 8
)(
    input clk,
    input reset,
    input load,
    input reg [WIDTH-1:0] load_value,
    output reg [WIDTH-1:0] count
);
    always @(posedge clk) begin
        if (reset)
            count <= {WIDTH{1'b0}};
        else if (load)
            count <=load_value;
        else
            count <= count + 1;
    end
endmodule
