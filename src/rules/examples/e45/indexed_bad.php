<?php

function indexed_bad(array $items): array
{
    $rows = [];

    foreach ($items as $index => $item) {
        $rows[$index] = $item;
    }

    return $rows;
}
