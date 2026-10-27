<?php

/**
 * @param list<int> $items
 */
function list_unknown_index(array $items, string $raw): ?int
{
    $index = (int) $raw;

    return $items[$index];
}
