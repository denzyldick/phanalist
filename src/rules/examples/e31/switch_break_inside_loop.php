<?php

function switch_break_inside_loop(int $value): void
{
    while ($value > 0) {
        switch ($value) {
            case 1:
                break;
            default:
                return;
        }

        $value = $value - 1;
    }

    echo $value;
}
