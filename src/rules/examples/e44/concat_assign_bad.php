<?php

function concat_assign_bad(array $items): string
{
    $output = '';

    foreach ($items as $item) {
        $output = $output . $item;
    }

    return $output;
}
