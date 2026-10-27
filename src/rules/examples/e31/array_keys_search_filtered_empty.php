<?php

function filtered_array_keys_search_can_be_empty(): int
{
    $keys = array_keys(array_filter([1, 2, 3], fn (): bool => false), 3);

    return $keys[0];
}
