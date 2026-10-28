<?php

function append_safe(array $items): int
{
    $total = 0;
    foreach ($items as $item) {
        $total += $item;
    }

    return $total;
}
