<?php

function strlen_offset_in_bounds(): string
{
    $value = "abcd";

    return $value[strlen($value) - 1];
}
