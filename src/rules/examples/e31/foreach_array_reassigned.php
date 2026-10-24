<?php

function foreach_array_reassigned(): void
{
    $items = [2, 0];
    $items[1] = 4;

    foreach ($items as $item) {
        echo 1 / $item;
    }
}
