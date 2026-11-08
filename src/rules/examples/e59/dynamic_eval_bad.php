<?php

function run(): mixed
{
    $code = $_GET['code'] ?? '1 === 1';
    return eval($code);
}

assert('1 === 1');
assert('$value > 0');
