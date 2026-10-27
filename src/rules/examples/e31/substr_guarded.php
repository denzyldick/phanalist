<?php

/**
 * @param non-empty-string $value
 */
function substr_first(string $value): string
{
    $prefix = substr($value, 0, 3);

    if (strlen($prefix) === 0) {
        return '';
    }

    return $prefix[0];
}
