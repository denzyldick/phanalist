<?php

function copy_safe(array $items): int
{
    $sum = 0;
    foreach ($items as $item) {
        $sum += $item;
    }

    return $sum;
}
