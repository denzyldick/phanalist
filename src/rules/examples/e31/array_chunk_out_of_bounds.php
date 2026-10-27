<?php

/**
 * @param list<int> $items
 */
function array_chunk_tail(array $items): int
{
    $chunks = array_chunk($items, 2);

    return $chunks[count($items)];
}
