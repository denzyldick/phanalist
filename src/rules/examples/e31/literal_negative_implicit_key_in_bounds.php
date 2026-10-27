<?php

function negative_implicit_key_is_in_bounds(): string
{
    $values = [-5 => 'first', 'second'];

    return $values[-4];
}
