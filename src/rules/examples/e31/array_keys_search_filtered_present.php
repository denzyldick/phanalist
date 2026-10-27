<?php

function filtered_array_keys_search_keeps_matching_keys(): int
{
    $keys = array_keys(array_filter([1, 2, 3], fn (): bool => true), 2);

    return $keys[0];
}
