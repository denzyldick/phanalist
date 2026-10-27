<?php

/**
 * @param non-empty-list<int> $left
 * @param non-empty-list<int> $right
 */
function array_merge_out_of_bounds(array $left, array $right): int
{
    $merged = array_merge($left, $right);

    return $merged[count($left) + count($right)];
}
