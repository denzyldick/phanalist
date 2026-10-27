<?php

function array_filter_without_a_callback_keeps_truthy_values(): int
{
    $values = array_filter([0, 1, 2]);
    $keys = array_keys($values);

    return $keys[1];
}
