<?php

function count_array_out_of_bounds(): int
{
    $items = [10, 20, 30];

    return $items[count($items)];
}
