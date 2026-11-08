.equ    arr_size, 10

        .data
arr:    .word 1, 1, 2, 2, 2, 5, 5, 7, 8, 9
res:    .org 16
        .text
start:
    MOV R1, R0
    LOAD R2, arr(R1)    ; Хранит последнее число
    STORE R2, res(R0)   ; Записываем первое число в результат, 16 + 0

loop:
    ADDI R1, 1
    LOAD R3, arr(R1)

    CMP R2, R3
    BEQ skip            ; Если числа равны, то просто пропускаем запись, в памяти и так ноль

    STORE R3, res(R1)   ; Записываем в 16 + R1

skip:
    ; Запоминаем новое предыдущее число
    LOAD R2, R3
    ; Проверка, что еще не достигли конца массива
    CMPI R1, arr_size-1
    BNE loop
    HALT
