<?php

class NeverReturnMethod
{
    public static function stopHere(): never
    {
        throw new RuntimeException();
    }

    public static function call(): void
    {
        self::stopHere();

        echo 1;
    }
}
