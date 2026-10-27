<?php

function array_keys_search_keeps_matching_keys(): string
{
    $values = ['first' => 1, 'second' => 2];
    $keys = array_keys($values, 2);

    return $keys[0];
}
