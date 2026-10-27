<?php

/**
 * @param list<int> $items
 */
function array_slice_first(array $items): ?int
{
    $head = array_slice($items, 0, 1);

    if ($head === []) {
        return null;
    }

    return $head[0];
}
