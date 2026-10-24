<?php

namespace App;

trait ZeroTrait
{
    public static function zero(): int
    {
        return 0;
    }
}

class UsesZeroTrait
{
    use ZeroTrait;
}

function use_trait_method(): float
{
    return 1 / UsesZeroTrait::zero();
}
