<?php

function consume(array $items): void
{
    if ($items === []) {
        return;
    }

    array_pop($items);
    consume($items);
}
