<?php

namespace App;

class BaseDivisors
{
    public static function zero(): int
    {
        return 0;
    }
}

class ChildDivisors extends BaseDivisors
{
}

function use_inherited_method(): float
{
    return 1 / ChildDivisors::zero();
}
