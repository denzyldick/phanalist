<?php

function array_key_last_past_end(): int
{
    $values = array_values([10, 20, 30]);
    $keys = [10, 20, 30];

    return $values[array_key_last($keys) + 1];
}
