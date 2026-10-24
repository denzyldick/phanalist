<?php

function array_missing_string_key(): int
{
    $values = ['first' => 10];

    return $values['second'];
}
