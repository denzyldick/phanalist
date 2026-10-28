<?php

function retain_bad(array $items): array
{
    $rows = [];
    foreach ($items as $item) {
        $rows[] = $item;
    }

    return $rows;
}
