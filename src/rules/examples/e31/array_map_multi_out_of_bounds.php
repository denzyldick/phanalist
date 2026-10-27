<?php

/**
 * @param list<int> $left
 * @param list<int> $right
 */
function array_map_multi_tail(array $left, array $right): int
{
    $mapped = array_map(null, $left, $right);

    return $mapped[count($left) + count($right)];
}
