<?php

function inline_array_keys_search_can_be_empty(): int
{
    $keys = array_keys([1, 2, 3], 4);

    return $keys[0];
}
