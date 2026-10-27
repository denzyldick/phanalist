<?php

function array_filter_callback_can_remove_every_element(): int
{
    $values = array_filter([1, 2, 3], fn (): bool => false);
    $keys = array_keys($values);

    return $keys[0];
}
