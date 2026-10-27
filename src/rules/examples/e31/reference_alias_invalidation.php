<?php

function alias_assignment_does_not_keep_stale_values(): int
{
    $values = [1 => 0];
    $alias = &$values;
    $alias[1] = 5;

    return 10 / $values[1];
}

function element_reference_does_not_keep_stale_values(): int
{
    $values = [1 => 0];
    $reference = &$values[1];
    $reference = 5;

    return 10 / $values[1];
}

function foreach_reference_does_not_keep_stale_values(): int
{
    $values = [1 => 0];

    foreach ($values as &$value) {
        $value = 5;
    }

    return 10 / $values[1];
}
