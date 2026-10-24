<?php

class PhpdocAssertion
{
    /**
     * @psalm-assert non-zero-int $value
     */
    public static function assumeNonZero($value): void
    {
    }

    public static function divide(int $divisor): float
    {
        self::assumeNonZero($divisor);

        return 1 / $divisor;
    }
}
