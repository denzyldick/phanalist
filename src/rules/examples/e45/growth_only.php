<?php

function growth_only(array $items): array
{
    $rows = [];

    foreach ($items as $item) {
        $rows[] = 1;
    }

    return $rows;
}
