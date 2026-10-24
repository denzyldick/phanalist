<?php

function do_while_guarded(): float
{
    $index = 1;

    do {
        $result = 1 / $index;
        $index++;
    } while ($index < 3);

    return $result;
}
