<?php

function stream_safe(array $items): void
{
    foreach ($items as $item) {
        echo $item;
    }
}
