module counter4(
    input        clk,
    input        reset,
    input        load,
    input  [3:0] load_value,
    output [3:0] count
);
    counter #(.WIDTH(4)) u_counter (
        .clk       (clk),
        .reset     (reset),
        .load      (load),
        .load_value(load_value),
        .count     (count)
    );
endmodule
