<?php

function null_property_access(): mixed
{
    $value = null;

    return $value->name;
}
