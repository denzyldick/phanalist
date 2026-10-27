<?php

function array_filter_callback_keeps_every_element(): int
{
    $values = array_filter([1, 2, 3], fn (int $value): bool => $value > 0);
    $keys = array_keys($values);

    return $keys[0];
}
