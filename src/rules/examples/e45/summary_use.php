<?php

function retain_summary_use(array $items): array
{
    $rows = [];
    foreach ($items as $item) {
        $rows[] = retain_helper($items);
    }

    return $rows;
}
