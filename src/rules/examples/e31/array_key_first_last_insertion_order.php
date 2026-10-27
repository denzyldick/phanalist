<?php

function array_key_first_out_of_order(): int
{
    $values = [1 => 10, 0 => 20];
    $map = [1 => 30];

    return $map[array_key_first($values)];
}

function array_key_last_out_of_order(): int
{
    $values = [1 => 10, 0 => 20];
    $map = [0 => 30];

    return $map[array_key_last($values)];
}

function array_key_first_associative(): int
{
    $values = ['b' => 10, 'a' => 20];
    $map = ['b' => 30];

    return $map[array_key_first($values)];
}

function array_key_last_associative(): int
{
    $values = ['b' => 10, 'a' => 20];
    $map = ['a' => 30];

    return $map[array_key_last($values)];
}
