<?php

/**
 * @param list<int> $items
 */
function list_direct_past_end(array $items): int
{
    return $items[count($items)];
}

/**
 * @param list<int> $items
 */
function list_loop_past_end(array $items): int
{
    $sum = 0;
    for ($i = 0; $i <= count($items); $i++) {
        $sum += $items[$i];
    }

    return $sum;
}

/**
 * @param non-empty-list<int> $items
 */
function list_negative_index(array $items): int
{
    return $items[-1];
}
