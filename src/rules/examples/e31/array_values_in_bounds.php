<?php

/**
 * @param non-empty-list<int> $items
 */
function array_values_in_bounds(array $items): int
{
    $values = array_values($items);

    return $values[count($items) - 1];
}
