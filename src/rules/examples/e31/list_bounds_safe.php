<?php

/**
 * @param list<int> $items
 */
function list_loop_in_bounds(array $items): int
{
    $sum = 0;
    for ($i = 0; $i < count($items); $i++) {
        $sum += $items[$i];
    }

    return $sum;
}

/**
 * @param non-empty-list<int> $items
 */
function list_last_in_bounds(array $items): int
{
    return $items[count($items) - 1];
}

/**
 * @param list<int> $items
 */
function list_guard_in_bounds(array $items, int $index): ?int
{
    if ($index >= 0 && $index < count($items)) {
        return $items[$index];
    }

    return null;
}

/**
 * @param list<int> $items
 */
function list_append_in_bounds(array $items): int
{
    $items[] = 42;

    return $items[count($items) - 1];
}

/**
 * @param list<int> $items
 */
function list_short_circuit_in_bounds(array $items, int $index, bool $enabled): ?int
{
    if ($enabled && $index >= 0 && $index < count($items)) {
        return $items[$index];
    }

    return null;
}

/**
 * @param list<int> $items
 */
function list_foreach_short_circuit_in_bounds(array $items, bool $enabled): int
{
    $count = count($items);

    foreach ($items as $i => $value) {
        if ($enabled && $i + 1 < $count) {
            return $items[$i + 1];
        }
    }

    return 0;
}
