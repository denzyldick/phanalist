<?php

/**
 * @param non-empty-string $value
 */
function substr_tail(string $value): string
{
    $prefix = substr($value, 0, 3);

    return $prefix[3];
}
