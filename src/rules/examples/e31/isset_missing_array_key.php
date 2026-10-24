<?php

function isset_missing_array_key(): bool
{
    $values = [1, 2];

    return isset($values[5]);
}
