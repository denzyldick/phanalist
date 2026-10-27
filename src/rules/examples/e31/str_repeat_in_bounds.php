<?php

/**
 * @param non-empty-string $value
 */
function str_repeat_in_bounds(string $value): string
{
    $repeated = str_repeat($value, 2);

    return $repeated[strlen($value) * 2 - 1];
}
