<?php

/**
 * @param non-empty-string $value
 */
function str_split_first(string $value): string
{
    $parts = str_split($value);

    return $parts[0];
}
