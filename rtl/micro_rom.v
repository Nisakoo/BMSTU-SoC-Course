// ROM микрокоманд: по коду операции и номеру шага выдаёт слово микрокоманды.
// Таблицы: docs/ISA.md (раздел «Команды», «Слово микрокоманды», «Микропрограммы»).
module micro_rom(
    input      [5:0]  opcode,   // IR[31:26] или FETCH в фазе выборки
    input      [3:0]  step,     // номер шага, mPC
    output reg [21:0] ctrl      // слово микрокоманды
);
    // ---------- коды операций ----------
    localparam [5:0] OP_HALT   = 6'h00;
    localparam [5:0] OP_ADD    = 6'h01;
    localparam [5:0] OP_SUB    = 6'h02;
    localparam [5:0] OP_AND    = 6'h03;
    localparam [5:0] OP_OR     = 6'h04;
    localparam [5:0] OP_XOR    = 6'h05;
    localparam [5:0] OP_NOT    = 6'h06;
    localparam [5:0] OP_SHL    = 6'h07;
    localparam [5:0] OP_SHR    = 6'h08;
    localparam [5:0] OP_SAR    = 6'h09;
    localparam [5:0] OP_CMP    = 6'h0A;
    localparam [5:0] OP_ADDI   = 6'h11;
    localparam [5:0] OP_SUBI   = 6'h12;
    localparam [5:0] OP_ANDI   = 6'h13;
    localparam [5:0] OP_ORI    = 6'h14;
    localparam [5:0] OP_XORI   = 6'h15;
    localparam [5:0] OP_SHLI   = 6'h17;
    localparam [5:0] OP_SHRI   = 6'h18;
    localparam [5:0] OP_SARI   = 6'h19;
    localparam [5:0] OP_CMPI   = 6'h1A;
    localparam [5:0] OP_LOAD   = 6'h20;
    localparam [5:0] OP_STORE  = 6'h21;
    localparam [5:0] OP_JUMP   = 6'h30;
    localparam [5:0] OP_BEQ    = 6'h31;
    localparam [5:0] OP_BNE    = 6'h32;
    localparam [5:0] OP_BLTU   = 6'h33;
    localparam [5:0] OP_BGEU   = 6'h34;
    localparam [5:0] OP_JAL    = 6'h35;
    localparam [5:0] OP_JR     = 6'h36;
    localparam [5:0] OP_HRT    = 6'h38;   // HIGH_RES_TICK
    localparam [5:0] OP_FETCH  = 6'h3F;

    // ---------- сигналы слова микрокоманды (положение битов: ISA.md) ----------
    localparam [21:0] LAST        = 22'd1 << 21;
    localparam [21:0] HALT        = 22'd1 << 20;
    localparam [21:0] MEM_WE      = 22'd1 << 19;
    localparam [21:0] ADDR_WE     = 22'd1 << 18;
    // wb_sel [17:16]
    localparam [21:0] WB_CAPT     = 22'd0 << 16;
    localparam [21:0] WB_PC       = 22'd1 << 16;
    localparam [21:0] WB_MEM      = 22'd2 << 16;
    localparam [21:0] WB_ALU      = 22'd3 << 16;
    localparam [21:0] FLAGS_NOCAP = 22'd1 << 15;   // flags_sel = 1
    localparam [21:0] FLAGS_WE    = 22'd1 << 14;
    // alu_op [13:10]
    localparam [21:0] ALU_ADD     = 22'd0 << 10;
    localparam [21:0] ALU_SUB     = 22'd1 << 10;
    localparam [21:0] ALU_AND     = 22'd2 << 10;
    localparam [21:0] ALU_OR      = 22'd3 << 10;
    localparam [21:0] ALU_XOR     = 22'd4 << 10;
    localparam [21:0] ALU_NOT     = 22'd5 << 10;
    localparam [21:0] ALU_SHL     = 22'd6 << 10;
    localparam [21:0] ALU_SHR     = 22'd7 << 10;
    localparam [21:0] ALU_SAR     = 22'd8 << 10;
    localparam [21:0] B_IMM       = 22'd1 << 9;    // alu_b_sel = 1
    localparam [21:0] REG_WE      = 22'd1 << 8;
    localparam [21:0] RS2_RD      = 22'd1 << 7;    // rs2_sel = 1
    localparam [21:0] IR_WE       = 22'd1 << 6;
    // pc_cond [5:3]
    localparam [21:0] COND_ALWAYS = 22'd0 << 3;
    localparam [21:0] COND_Z1     = 22'd1 << 3;
    localparam [21:0] COND_Z0     = 22'd2 << 3;
    localparam [21:0] COND_C1     = 22'd3 << 3;
    localparam [21:0] COND_C0     = 22'd4 << 3;
    // pc_src [2:1]
    localparam [21:0] PC_NEXT     = 22'd0 << 1;    // PC + 1
    localparam [21:0] PC_REL      = 22'd1 << 1;    // PC + imm
    localparam [21:0] PC_REG      = 22'd2 << 1;    // rs1
    localparam [21:0] PC_WE       = 22'd1;

    // ---------- общие части микропрограмм ----------
    // результат АЛУ в rd, флаги обновляются, команда закончена
    localparam [21:0] ALU_WRITE = WB_ALU | REG_WE | FLAGS_WE | LAST;
    // первый шаг LOAD и STORE: адрес = rs1 + imm, запоминается в регистре адреса
    localparam [21:0] ADDR_CALC = ALU_ADD | B_IMM | ADDR_WE;
    // условный или безусловный переход на PC + imm
    localparam [21:0] JUMP_REL  = PC_WE | PC_REL | LAST;

    always @* begin
        // неизвестный код или шаг останавливает ядро; значение по умолчанию
        // также исключает защёлки
        ctrl = HALT | LAST;

        case ({opcode, step})
            // выборка
            {OP_FETCH, 4'd0}: ctrl = IR_WE | PC_WE | PC_NEXT | COND_ALWAYS | LAST;

            {OP_HALT,  4'd0}: ctrl = HALT | LAST;

            // команды АЛУ с регистром rs2
            {OP_ADD,   4'd0}: ctrl = ALU_ADD | ALU_WRITE;
            {OP_SUB,   4'd0}: ctrl = ALU_SUB | ALU_WRITE;
            {OP_AND,   4'd0}: ctrl = ALU_AND | ALU_WRITE;
            {OP_OR,    4'd0}: ctrl = ALU_OR  | ALU_WRITE;
            {OP_XOR,   4'd0}: ctrl = ALU_XOR | ALU_WRITE;
            {OP_NOT,   4'd0}: ctrl = ALU_NOT | ALU_WRITE;
            {OP_SHL,   4'd0}: ctrl = ALU_SHL | ALU_WRITE;
            {OP_SHR,   4'd0}: ctrl = ALU_SHR | ALU_WRITE;
            {OP_SAR,   4'd0}: ctrl = ALU_SAR | ALU_WRITE;
            {OP_CMP,   4'd0}: ctrl = ALU_SUB | FLAGS_WE | LAST;

            // команды АЛУ с константой imm
            {OP_ADDI,  4'd0}: ctrl = ALU_ADD | B_IMM | ALU_WRITE;
            {OP_SUBI,  4'd0}: ctrl = ALU_SUB | B_IMM | ALU_WRITE;
            {OP_ANDI,  4'd0}: ctrl = ALU_AND | B_IMM | ALU_WRITE;
            {OP_ORI,   4'd0}: ctrl = ALU_OR  | B_IMM | ALU_WRITE;
            {OP_XORI,  4'd0}: ctrl = ALU_XOR | B_IMM | ALU_WRITE;
            {OP_SHLI,  4'd0}: ctrl = ALU_SHL | B_IMM | ALU_WRITE;
            {OP_SHRI,  4'd0}: ctrl = ALU_SHR | B_IMM | ALU_WRITE;
            {OP_SARI,  4'd0}: ctrl = ALU_SAR | B_IMM | ALU_WRITE;
            {OP_CMPI,  4'd0}: ctrl = ALU_SUB | B_IMM | FLAGS_WE | LAST;

            // обращение к памяти данных: два шага
            {OP_LOAD,  4'd0}: ctrl = ADDR_CALC;
            {OP_LOAD,  4'd1}: ctrl = WB_MEM | REG_WE | LAST;
            {OP_STORE, 4'd0}: ctrl = ADDR_CALC;
            {OP_STORE, 4'd1}: ctrl = RS2_RD | MEM_WE | LAST;

            // переходы
            {OP_JUMP,  4'd0}: ctrl = JUMP_REL | COND_ALWAYS;
            {OP_BEQ,   4'd0}: ctrl = JUMP_REL | COND_Z1;
            {OP_BNE,   4'd0}: ctrl = JUMP_REL | COND_Z0;
            {OP_BLTU,  4'd0}: ctrl = JUMP_REL | COND_C1;
            {OP_BGEU,  4'd0}: ctrl = JUMP_REL | COND_C0;
            {OP_JAL,   4'd0}: ctrl = WB_PC | REG_WE | JUMP_REL | COND_ALWAYS;
            {OP_JR,    4'd0}: ctrl = PC_WE | PC_REG | COND_ALWAYS | LAST;

            // HIGH_RES_TICK
            {OP_HRT,   4'd0}: ctrl = WB_CAPT | REG_WE | FLAGS_NOCAP | FLAGS_WE | LAST;

            default: ctrl = HALT | LAST;
        endcase
    end
endmodule
