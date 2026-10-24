<?php

function foreach_key_value(): void
{
    $items = ['first' => 2, 'second' => 4];

    foreach ($items as $key => $item) {
        echo $key;
        echo 1 / $item;
    }
}
