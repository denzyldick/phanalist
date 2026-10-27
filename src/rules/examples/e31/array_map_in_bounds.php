<?php

/**
 * @param non-empty-list<int> $items
 */
function array_map_in_bounds(array $items): int
{
    $mapped = array_map(static fn (int $value): int => $value + 1, $items);

    return $mapped[count($items) - 1];
}
