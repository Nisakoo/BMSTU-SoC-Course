/* verilator lint_off DECLFILENAME */
module soc_top #(
    parameter PROGRAM_FILE = "",       // hex-файл программы для ROM инструкций
    parameter DATA_FILE    = ""        // hex-файл начальных данных RAM
)(
    input         clk,
    input         reset,

    // выводы устройств: Rust ставит входы и читает выходы
    input         btn_react,           // кнопка реакции (захват)
    input  [1:0]  remote,              // кнопки пульта: бит 0 — открыть, бит 1 — закрыть
    input  [1:0]  limit,               // концевые выключатели: бит 0 — открыто, бит 1 — закрыто
    input         ir_sensor,           // ИК-датчик препятствия
    output [7:0]  led,                 // светодиоды
    output        gate_open,           // привод ворот: открыть
    output        gate_close,          // привод ворот: закрыть
    input  [11:0] frame_addr,          // адрес слова кадра
    output [31:0] frame_data,          // слово кадра: 4 пикселя
    input  [9:0]  ram_view_addr,       // адрес слова RAM для просмотра
    output [31:0] ram_view_data,       // слово RAM

    // Rust останавливает симуляцию, когда видит 1
    output        halt
);
    // ---------- код операции выборки ----------
    localparam [5:0] OP_FETCH = 6'h3F;

    // ---------- слово микрокоманды ----------
    wire [21:0] ctrl;

    wire        last      = ctrl[21];
    wire        mem_we    = ctrl[19];
    wire        addr_we   = ctrl[18];
    wire [1:0]  wb_sel    = ctrl[17:16];
    wire        flags_sel = ctrl[15];
    wire        flags_we  = ctrl[14];
    wire [3:0]  alu_op    = ctrl[13:10];
    wire        alu_b_sel = ctrl[9];
    wire        reg_we    = ctrl[8];
    wire        rs2_sel   = ctrl[7];
    wire        ir_we     = ctrl[6];
    wire [2:0]  pc_cond   = ctrl[5:3];
    wire [1:0]  pc_src    = ctrl[2:1];
    wire        pc_we     = ctrl[0];

    assign halt = ctrl[20];

    // ---------- блок управления: фаза, шаг, ROM микрокоманд ----------
    wire        phase;                 // 0 — выборка, 1 — выполнение
    wire [3:0]  mpc;                   // номер шага
    wire [31:0] ir;                    // регистр команды

    reg_we #(.WIDTH(1)) u_phase (
        .clk(clk), .reset(reset), .we(last), .d(~phase), .q(phase)
    );

    counter #(.WIDTH(4)) u_mpc (
        .clk(clk), .reset(reset | last), .load(1'b0), .load_value(4'd0), .count(mpc)
    );

    wire [5:0] micro_op = phase ? ir[31:26] : OP_FETCH;

    micro_rom u_micro_rom (
        .opcode(micro_op), .step(mpc), .ctrl(ctrl)
    );

    // ---------- выборка команды ----------
    wire [9:0]  pc;
    wire [9:0]  pc_next;
    wire        pc_en;
    wire [31:0] rom_data;
    /* verilator lint_off UNUSEDSIGNAL */
    wire [3:0]  flags;                 // {N, Z, C, V}; переходы используют только Z и C
    /* verilator lint_on UNUSEDSIGNAL */

    rom #(.INIT_FILE(PROGRAM_FILE)) u_rom (
        .addr(pc), .data(rom_data)
    );

    reg_we #(.WIDTH(32)) u_ir (
        .clk(clk), .reset(reset), .we(ir_we), .d(rom_data), .q(ir)
    );

    reg_we #(.WIDTH(10)) u_pc (
        .clk(clk), .reset(reset), .we(pc_en), .d(pc_next), .q(pc)
    );

    // ---------- регистры, АЛУ ----------
    wire [31:0] imm32 = {{14{ir[17]}}, ir[17:0]};
    wire [3:0]  ra2   = rs2_sel ? ir[25:22] : ir[17:14];

    wire [31:0] rd1, rd2;
    wire [31:0] alu_b = alu_b_sel ? imm32 : rd2;
    wire [31:0] alu_r;
    wire [3:0]  alu_nzcv;

    wire [31:0] capt;                  // захват: значение для HIGH_RES_TICK
    wire        nocap;                 // захват: 1 — захвата не было
    wire [31:0] mem_rd;                // чтение памяти данных

    reg [31:0] wd;                     // данные для записи в регистр
    always @* begin
        case (wb_sel)
            2'd0:    wd = capt;
            2'd1:    wd = {22'd0, pc};
            2'd2:    wd = mem_rd;
            default: wd = alu_r;
        endcase
    end

    regfile u_regfile (
        .clk(clk), .reset(reset),
        .ra1(ir[21:18]), .ra2(ra2), .wa(ir[25:22]), .wd(wd), .we(reg_we),
        .rd1(rd1), .rd2(rd2)
    );

    alu u_alu (
        .a(rd1), .b(alu_b), .op(alu_op), .r(alu_r), .nzcv(alu_nzcv)
    );

    // флаги: из АЛУ или Z из NOCAP (HIGH_RES_TICK), остальные 0
    wire [3:0] flags_d = flags_sel ? {1'b0, nocap, 2'b00} : alu_nzcv;

    reg_we #(.WIDTH(4)) u_flags (
        .clk(clk), .reset(reset), .we(flags_we), .d(flags_d), .q(flags)
    );

    // ---------- логика PC ----------
    pc_logic u_pc_logic (
        .pc(pc), .imm(imm32[9:0]), .rs1(rd1[9:0]), .z(flags[2]), .c(flags[1]),
        .pc_we(pc_we), .pc_src(pc_src), .pc_cond(pc_cond),
        .next(pc_next), .we(pc_en)
    );

    // ---------- память данных ----------
    wire [31:0] addr;

    reg_we #(.WIDTH(32)) u_addr (
        .clk(clk), .reset(reset), .we(addr_we), .d(alu_r), .q(addr)
    );

    wire [9:0]  ram_addr, mmio_addr;
    wire [11:0] vram_addr;
    wire        ram_we, vram_we, mmio_we;
    wire [31:0] ram_rd, vram_rd, mmio_rd;

    addr_decoder u_decoder (
        .addr(addr), .we(mem_we),
        .ram_rd(ram_rd), .vram_rd(vram_rd), .mmio_rd(mmio_rd),
        .ram_addr(ram_addr), .ram_we(ram_we),
        .vram_addr(vram_addr), .vram_we(vram_we),
        .mmio_addr(mmio_addr), .mmio_we(mmio_we),
        .rd(mem_rd)
    );

    wire [9:0]  ram_raddr;
    wire [31:0] ram_rdata;

    ram #(.INIT_FILE(DATA_FILE)) u_ram (
        .clk(clk), .addr(ram_addr), .we(ram_we), .wd(rd2), .data(ram_rd),
        .raddr(ram_raddr), .rdata(ram_rdata)
    );

    wire [11:0] disp_raddr;
    wire [31:0] disp_rdata;

    vram u_vram (
        .clk(clk), .addr(vram_addr), .we(vram_we), .wd(rd2), .rd(vram_rd),
        .raddr(disp_raddr), .rdata(disp_rdata)
    );

    // ---------- устройства MMIO ----------
    // шина устройств: addr = mmio_addr, we = mmio_we, wd = rd2, чтение объединяется по ИЛИ
    wire [31:0] timer_rd, capture_rd, remote_rd, limit_rd, ir_sensor_rd, led_rd, gate_rd, display_rd;
    wire [31:0] ram_view_rd;
    wire [31:0] time_us;

    dev_timer #(.BASE(10'h000)) u_timer (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(timer_rd), .time_us(time_us)
    );

    dev_capture #(.BASE(10'h010)) u_capture (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(capture_rd), .btn(btn_react), .time_us(time_us),
        .capt(capt), .nocap(nocap)
    );

    dev_button #(.BASE(10'h020), .WIDTH(2)) u_remote (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(remote_rd), .btn(remote)
    );

    dev_button #(.BASE(10'h021), .WIDTH(2)) u_limit (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(limit_rd), .btn(limit)
    );

    dev_button #(.BASE(10'h022), .WIDTH(1)) u_ir_sensor (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(ir_sensor_rd), .btn(ir_sensor)
    );

    dev_led #(.BASE(10'h030), .WIDTH(8)) u_led (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(led_rd), .led(led)
    );

    dev_gate #(.BASE(10'h040)) u_gate (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(gate_rd), .gate_open(gate_open), .gate_close(gate_close)
    );

    dev_display u_display (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(display_rd),
        .vram_raddr(disp_raddr), .vram_rdata(disp_rdata),
        .frame_addr(frame_addr), .frame_data(frame_data)
    );

    dev_ram_view u_ram_view (
        .clk(clk), .reset(reset), .addr(mmio_addr), .we(mmio_we), .wd(rd2),
        .rd(ram_view_rd),
        .ram_raddr(ram_raddr), .ram_rdata(ram_rdata),
        .view_addr(ram_view_addr), .view_data(ram_view_data)
    );

    assign mmio_rd = timer_rd | capture_rd | remote_rd | limit_rd | ir_sensor_rd
                   | led_rd | gate_rd | display_rd | ram_view_rd;
endmodule
