<?php

namespace App;

class ParentCalc
{
    public static function value(): int
    {
        return 0;
    }
}

class ParentChildCalc extends ParentCalc
{
    public static function value(): int
    {
        return 2;
    }

    public static function useParent(): float
    {
        return 1 / parent::value();
    }
}
