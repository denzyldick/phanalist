<?php

/**
 * @param non-empty-list<int> $left
 * @param non-empty-list<int> $right
 */
function array_merge_in_bounds(array $left, array $right): int
{
    $merged = array_merge($left, $right);

    return $merged[count($left) + count($right) - 1];
}
