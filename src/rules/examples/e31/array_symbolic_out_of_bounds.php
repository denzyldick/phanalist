<?php

function array_symbolic_out_of_bounds(int $index): int
{
    $values = [10, 20, 30];

    return $values[$index];
}
