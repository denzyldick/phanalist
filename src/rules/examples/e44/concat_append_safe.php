<?php

function concat_append_safe(array $items): string
{
    $output = '';

    foreach ($items as $item) {
        $output .= $item;
    }

    return $output;
}
