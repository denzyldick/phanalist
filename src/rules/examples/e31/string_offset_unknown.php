<?php

/**
 * @param non-empty-string $value
 */
function string_offset_unknown(string $value, $index): string
{
    return $value[$index];
}
