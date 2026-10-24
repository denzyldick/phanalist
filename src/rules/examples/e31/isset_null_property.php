<?php

function isset_null_property(): bool
{
    $value = null;

    return isset($value->name);
}
