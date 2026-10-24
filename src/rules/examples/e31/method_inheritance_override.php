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
    public static function zero(): int
    {
        return 2;
    }
}

function use_overridden_method(): float
{
    return 1 / ChildDivisors::zero();
}
