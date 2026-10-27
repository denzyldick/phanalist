<?php

function array_fill_named(): int
{
    $filled = array_fill(count: 2, start_index: 0, value: 0);

    return $filled[0];
}
