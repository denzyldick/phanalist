<?php

namespace App;

class ParentCalc
{
    public static function zero(): int
    {
        return 0;
    }
}

trait ParentTrait
{
    public static function two(): int
    {
        return 2;
    }
}

class ParentChildCalc extends ParentCalc
{
    use ParentTrait;

    public static function useParent(): float
    {
        return 1 / parent::zero();
    }
}
