// Обёртка для тестов: ROM, загруженная из tests/data/rom_test.hex
module rom_program(
    input  [9:0]  addr,
    output [31:0] data
);
    rom #(.INIT_FILE("tests/data/rom_test.hex")) u_rom (
        .addr(addr),
        .data(data)
    );
endmodule
