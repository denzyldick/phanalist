<?php

/**
 * @param array<string, int> $map
 */
function array_keys_tail(array $map): string
{
    $keys = array_keys($map);

    return $keys[count($map)];
}
