<?php

function run(string $code): mixed
{
    return eval($code);
}

assert('1 === 1');
assert('$value > 0');
