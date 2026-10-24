<?php

function switch_continue_two(int $value): void
{
    while ($value > 0) {
        switch ($value) {
            default:
                continue 2;
        }

        1 / 0;
    }

    echo $value;
}
