<?php

function array_keys_search_can_be_empty(): int
{
    $values = ['first' => 1, 'second' => 2];
    $keys = array_keys($values, 3);

    return $keys[0];
}
