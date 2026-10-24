<?php

function divide_guarded(int $divisor): float
{
    if ($divisor !== 0) {
        return 1 / $divisor;
    }

    return 0.0;
}
