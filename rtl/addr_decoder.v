module addr_decoder(
    input  [31:0] addr,       // адрес данных из регистра адреса
    input         we,         // запись в память данных (mem_we)
    input  [31:0] ram_rd,     // чтение из RAM
    input  [31:0] vram_rd,    // чтение из видеопамяти
    input  [31:0] mmio_rd,    // чтение из MMIO
    output [9:0]  ram_addr,
    output        ram_we,
    output [11:0] vram_addr,
    output        vram_we,
    output [9:0]  mmio_addr,
    output        mmio_we,
    output reg [31:0] rd      // прочитанное слово (MEM_RD)
);
    wire ram_sel  = (addr[31:10] == 22'h000000);
    wire vram_sel = (addr[31:12] == 20'h00001);
    wire mmio_sel = (addr[31:10] == 22'h3FFFFF);

    // адрес внутри выбранной области: младшие биты адреса
    assign ram_addr  = addr[9:0];
    assign vram_addr = addr[11:0];
    assign mmio_addr = addr[9:0];

    // запись разрешается только выбранной области
    assign ram_we  = we & ram_sel;
    assign vram_we = we & vram_sel;
    assign mmio_we = we & mmio_sel;

    always @* begin
        if (ram_sel)       rd = ram_rd;
        else if (vram_sel) rd = vram_rd;
        else if (mmio_sel) rd = mmio_rd;
        else               rd = 32'd0;
    end
endmodule
