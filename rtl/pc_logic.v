// Логика PC: выбирает адрес следующей команды и решает, записывать ли его в PC.
// Комбинационный блок. Правила: docs/ISA.md, раздел «Переходы».
module pc_logic(
    input  [9:0] pc,        // текущий PC
    input  [9:0] imm,       // младшие 10 бит imm: адрес считается по модулю 1024
    input  [9:0] rs1,       // младшие 10 бит rs1 (для JR)
    input        z,         // флаг Z
    input        c,         // флаг C
    input        pc_we,     // разрешение записи PC из микрокоманды
    input  [1:0] pc_src,    // 0: PC + 1, 1: PC + imm, 2: rs1, 3: не используется
    input  [2:0] pc_cond,   // 0: всегда, 1: Z=1, 2: Z=0, 3: C=1, 4: C=0, 5-7: не используются
    output reg [9:0] next,  // адрес следующей команды
    output       we         // PC записывается: pc_we и условие выполнено
);
    reg cond_met;

    always @* begin
        case (pc_src)
            2'd0:    next = pc + 10'd1;
            2'd1:    next = pc + imm;
            2'd2:    next = rs1;
            default: next = pc;
        endcase
    end

    always @* begin
        case (pc_cond)
            3'd0:    cond_met = 1'b1;
            3'd1:    cond_met = z;
            3'd2:    cond_met = ~z;
            3'd3:    cond_met = c;
            3'd4:    cond_met = ~c;
            default: cond_met = 1'b0;
        endcase
    end

    // pc_src = 3 не определён: запись не выполняется
    assign we = pc_we & cond_met & (pc_src != 2'd3);
endmodule
