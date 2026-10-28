<?php

function retain_helper(array $items): array
{
    $rows = [];
    foreach ($items as $item) {
        $rows[] = $item;
    }

    return $rows;
}
