<?php

function do_while_second_iteration(): float
{
    $index = 0;

    do {
        $index++;
        if ($index === 2) {
            return 1 / ($index - 2);
        }
    } while ($index < 3);

    return 1.0;
}
