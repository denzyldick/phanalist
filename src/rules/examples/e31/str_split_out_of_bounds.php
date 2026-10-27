<?php

/**
 * @param non-empty-string $value
 */
function str_split_tail(string $value): string
{
    $parts = str_split($value);

    return $parts[strlen($value)];
}
