// Описание работы АЛУ находится в docs/ALU.md
module alu(
    input [31:0] a,
    input [31:0] b,
    input [3:0] op,
    output reg [31:0] r,
    output reg [3:0] nzcv
);
    localparam OP_ADD   = 4'b0000;
    localparam OP_SUB   = 4'b0001;
    localparam OP_AND   = 4'b0010;
    localparam OP_OR    = 4'b0011;
    localparam OP_XOR   = 4'b0100;
    localparam OP_NOT   = 4'b0101;
    localparam OP_SHL   = 4'b0110;
    localparam OP_SHR   = 4'b0111;
    localparam OP_SAR   = 4'b1000;

    reg         c, v;
    reg [32:0]  wide;

    always @* begin
        wide = 33'd0;
        c = 1'b0;
        v = 1'b0;
        r = 32'd0;

        case (op)
            OP_ADD: begin
                wide = {1'b0, a} + {1'b0, b};
                r = wide[31:0];
                c = wide[32];
                // Проверка знакого переполнения
                // Были одного знака => результат того же знака
                v = (a[31] == b[31]) && (r[31] != a[31]);
            end
            OP_SUB: begin
                wide = {1'b0, a} - {1'b0, b};
                r = wide[31:0];
                // Если a >= b, то r >= 0 => wide[32] == 0
                // Если a < b, то будет заем => wide[32] == 1
                c = wide[32];
                // Если a и b одного знака, то переполнения быть не может
                // (вычитаение модулей, легко понять, что результат будет в диапазоне)
                // Разные знаки: a - b = a + (-b), то  есть знаки (-b) и a совпадают,
                // а значит знак должен сохранится, если такого не было, то произошло переполнение
                v = (a[31] != b[31]) && (r[31] != a[31]);
            end
            OP_AND: r = a & b;
            OP_OR:  r = a | b;
            OP_XOR: r = a ^ b;
            OP_NOT: r = ~a;
            OP_SHL: r = a << b[4:0];
            OP_SHR: r = a >> b[4:0];
            OP_SAR: r = $unsigned($signed(a) >>> b[4:0]);
            default: r = 32'd0;
        endcase
    end

    assign nzcv = {r[31], (r == 32'd0), c, v};
endmodule
