module alu(
    input [31:0] a,
    input [31:0] b,
    input [3:0] op,
    output reg [31:0] result
);
    localparam OP_ADD = 4'd0;
    localparam OP_SUB = 4'd1;

    always @* begin
        case (op)
            OP_ADD: result = a + b;
            OP_SUB: result = a - b;
            default: result = 32'd0;
        endcase
    end
endmodule
