<?php

/**
 * @param non-empty-list<int> $items
 */
function array_reverse_in_bounds(array $items): int
{
    $reversed = array_reverse($items);

    return $reversed[count($items) - 1];
}
