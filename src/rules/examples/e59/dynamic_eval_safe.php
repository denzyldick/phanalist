<?php

function run(callable $callback): mixed
{
    return $callback();
}

$value = 1;
assert($value === 1);
assert(is_int($value));
