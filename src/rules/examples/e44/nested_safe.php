<?php

function nested_safe(array $items): void
{
    foreach ($items as $item) {
        echo $item;
    }
}
