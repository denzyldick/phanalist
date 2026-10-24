<?php

function coalesce_null_property(): string
{
    $value = null;

    return $value->name ?? 'fallback';
}
