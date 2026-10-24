<?php

function foreach_array_guarded(): void
{
    $items = [2, 4, 8];

    foreach ($items as $item) {
        echo 1 / $item;
    }
}
