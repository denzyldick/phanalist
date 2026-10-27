<?php

function array_fill_tail(int $count): int
{
    $filled = array_fill(0, $count, 0);

    return $filled[$count];
}
