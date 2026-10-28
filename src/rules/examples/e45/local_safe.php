<?php

function local_safe(array $items): int
{
    $temporary = [];

    foreach ($items as $item) {
        $temporary[] = $item;
    }

    return count($temporary);
}
