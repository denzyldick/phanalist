<?php

function strlen_offset_out_of_bounds(): string
{
    $value = "abcd";

    return $value[strlen($value)];
}
