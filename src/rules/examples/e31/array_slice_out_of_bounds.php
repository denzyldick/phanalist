<?php

/**
 * @param list<int> $items
 */
function array_slice_tail(array $items): int
{
    $tail = array_slice($items, 0, 3);

    return $tail[3];
}
