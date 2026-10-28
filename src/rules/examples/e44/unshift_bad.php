<?php

function unshift_bad(array $items): array
{
    $copy = [];
    foreach ($items as $item) {
        array_unshift($copy, $item);
    }

    return $copy;
}
