<?php

function recursive_count_is_not_the_normal_count(): int
{
    $nested = [[1, 2], [3]];
    $values = [10, 20, 30, 40, 50];
    $count = count($nested, COUNT_RECURSIVE);

    return $values[$count - 3];
}
