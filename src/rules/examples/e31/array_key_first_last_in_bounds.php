<?php

function array_key_first_last_in_bounds(): int
{
    $values = [10, 20, 30];

    return $values[array_key_first($values)] + $values[array_key_last($values)];
}

function array_key_first_empty(): ?int
{
    $values = [];

    return array_key_first($values);
}
