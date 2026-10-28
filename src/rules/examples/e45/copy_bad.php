<?php

function copy_bad(array $items): array
{
    $copy = [];
    foreach ($items as $item) {
        $copy[] = $item;
    }

    return $copy;
}
