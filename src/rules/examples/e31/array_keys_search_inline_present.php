<?php

function inline_array_keys_search_keeps_matching_keys(): int
{
    $keys = array_keys([1, 2, 3], 2);

    return $keys[0];
}
