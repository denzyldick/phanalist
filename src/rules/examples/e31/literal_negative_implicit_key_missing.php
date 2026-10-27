<?php

function negative_implicit_key_is_not_zero(): string
{
    $values = [-5 => 'first', 'second'];

    return $values[0];
}
