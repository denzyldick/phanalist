<?php

function while_division_guarded(): void
{
    $index = 1;
    while ($index < 4) {
        $value = 1 / $index;
        $index++;
    }
}
