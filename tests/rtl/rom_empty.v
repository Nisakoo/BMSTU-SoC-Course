// Обёртка для тестов: ROM без файла (INIT_FILE по умолчанию пустой)
module rom_empty(
    input  [9:0]  addr,
    output [31:0] data
);
    rom u_rom (
        .addr(addr),
        .data(data)
    );
endmodule
