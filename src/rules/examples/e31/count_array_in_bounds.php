<?php

function count_array_in_bounds(): int
{
    $items = [10, 20, 30];

    return $items[count($items) - 1];
}
