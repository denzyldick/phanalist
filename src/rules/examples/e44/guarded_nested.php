<?php

function guarded_nested(array $items): void
{
    $limit = min(count($items), 10);
    for ($i = 0; $i < $limit; $i++) {
        for ($j = 0; $j < $limit; $j++) {
            echo $i + $j;
        }
    }
}
