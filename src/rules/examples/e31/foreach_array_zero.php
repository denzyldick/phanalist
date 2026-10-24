<?php

function foreach_array_zero(): void
{
    $items = [2, 0, 4];

    foreach ($items as $item) {
        echo 1 / $item;
    }
}
