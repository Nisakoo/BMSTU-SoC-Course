module counter8(
    input        clk,
    input        reset,
    input        load,
    input  [7:0] load_value,
    output [7:0] count
);
    counter #(.WIDTH(8)) u_counter (
        .clk       (clk),
        .reset     (reset),
        .load      (load),
        .load_value(load_value),
        .count     (count)
    );
endmodule
