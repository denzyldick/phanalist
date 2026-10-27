<?php

function array_key_first_empty_is_null(): int
{
    $values = [];

    if (array_key_first($values) !== null) {
        return 1 / 0;
    }

    return 0;
}

function array_key_last_empty_is_null(): int
{
    $values = [];

    if (array_key_last($values) !== null) {
        return 1 / 0;
    }

    return 0;
}
