<?php

function string_offset_out_of_bounds(): string
{
    $value = 'hello';

    return $value[5];
}
