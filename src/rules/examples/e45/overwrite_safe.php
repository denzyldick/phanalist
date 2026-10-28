<?php

function overwrite_safe(array $items): array
{
    foreach ($items as $key => $value) {
        $items[$key] = $value + 1;
    }

    return $items;
}
