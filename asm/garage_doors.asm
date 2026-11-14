.equ    REMOTE,     -992 ; Пульт управления
.equ    IR_SENSOR,  -990 ; ИК-датчик
.equ    LIMIT,      -991 ; Концевики
.equ    GATE,       -960 ; Ворота

; R5 хранит состояние ворот: стоят = 00, открываются = 01, закрываются = 10

loop:
    ; Опрос датчиков
    LOAD R1, REMOTE(R0)
    LOAD R2, LIMIT(R0)
    LOAD R4, IR_SENSOR(R0)
    ; Проверка состояния ворот
    CMPI R5, 0b01
    BEQ opening
    CMPI R5, 0b10
    BEQ closing

idle:
    ; Прыгаем сюда, если условия в loop не сработали
    ANDI R3, R1, 0b01 ; Проверяем нажата ли кнопка открыть
    BNE start_opening ; Если да, то прыгаем
    ANDI R3, R1, 0b10 ; Проверка нажата ли кнопка закрыть
    BNE start_closing ; Если да, то прыгаем
    JUMP loop

start_opening:
    ANDI R3, R2, 0b01 ; Проверка открыты ли ворота сейчас
    BNE loop ; Если да, то прыгаем
    LI R5, 0b01
    LI R3, 0b01 ; Команда на открытие ворот
    STORE R3, GATE(R0)
    JUMP loop

start_closing:
    ANDI R3, R2, 0b10 ; Проверка закрыты ли ворота сейчас
    BNE loop ; Если да, то прыгаем
    ANDI R3, R4, 1 ; Если ИК-датчик активен, то не включаем мотор
    BNE loop
    LI R5, 0b10
    LI R3, 0b10 ; Команда на закрытие ворот
    STORE R3, GATE(R0)
    JUMP loop

opening:
    ANDI R3, R2, 0b01 ; Проверка статуса концевика
    BEQ loop ; Если концевик еще не сработал, то прыгаем в loop, где будет повторное чтение
    JUMP stop_gate

closing:
    ANDI R3, R4, 1 ; Проверка сработал ли ИК-датчик
    BNE stop_gate ; Если ИК-датчик сработал, то останавливаем ворота
    ANDI R3, R2, 0b10 ; Проверка статуса концевика
    BEQ loop ; Если концевик еще не сработал, то прыгаем в loop, где будет повторное чтение
    JUMP stop_gate

stop_gate:
    STORE R0, GATE(R0) ; Останавливаем ворота
    MOV R5, R0  ; Глобальное состояние 0
    JUMP loop
