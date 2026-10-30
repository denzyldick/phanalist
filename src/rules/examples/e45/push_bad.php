<?php

function push_bad(array $items): array
{
    $rows = [];

    foreach ($items as $item) {
        array_push($rows, $item);
    }

    return $rows;
}
