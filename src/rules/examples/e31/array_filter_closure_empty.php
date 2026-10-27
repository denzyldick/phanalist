<?php

function array_filter_closure_can_remove_every_element(): int
{
    $values = array_filter([1, 2, 3], function (): bool {
        return false;
    });
    $keys = array_keys($values);

    return $keys[0];
}
