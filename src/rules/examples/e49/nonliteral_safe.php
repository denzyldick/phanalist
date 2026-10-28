<?php

function build(int $count, string $delimiter): array
{
    $repeat = str_repeat('x', $count);
    $fill = array_fill(0, $count, 'x');
    $parts = explode($delimiter, 'a,b,c');

    return [$repeat, $fill, $parts];
}
