<?php

function str_pad_tail(): string
{
    $padded = str_pad('abc', 10, ' ');

    return $padded[10];
}
