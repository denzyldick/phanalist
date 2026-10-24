<?php

function switch_break_two(int $value): void
{
    while ($value > 0) {
        switch ($value) {
            default:
                break 2;
        }

        1 / 0;
    }

    echo $value;
}
