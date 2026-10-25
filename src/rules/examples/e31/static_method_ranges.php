<?php

class CronLikeShifter
{
    public static function run(): int
    {
        return self::shift(3, 5);
    }

    private static function shift(int $min, int $max): int
    {
        return self::shiftAndGroupValues($min, $max);
    }

    private static function shiftAndGroupValues(int $min, int $max): int
    {
        return self::modulo($max - $min + 1);
    }

    private static function modulo(int $divisor): int
    {
        return 10 % $divisor;
    }
}

class PsyshLikeColumns
{
    public static function run(): int
    {
        return self::calculate(0);
    }

    private static function calculate(int $columns): int
    {
        return self::width(max(1, $columns));
    }

    private static function width(int $columns): int
    {
        return 100 % $columns;
    }
}

class BadRangeShifter
{
    public static function shift(): int
    {
        return self::modulo(0);
    }

    private static function modulo(int $divisor): int
    {
        return 10 % $divisor;
    }
}

class InstanceRangeShifter
{
    public function entry(): int
    {
        return $this->shift(0);
    }

    private function shift(int $columns): int
    {
        return $this->modulo(max(1, $columns));
    }

    private function modulo(int $divisor): int
    {
        return 10 % $divisor;
    }
}

class ClosureShadowRange
{
    public function entry(): int
    {
        return self::shift(0);
    }

    private static function shift(int $columns): int
    {
        $modulo = function (int $columns): int {
            return self::remainder($columns);
        };
        $modulo($columns);

        return self::remainder(1);
    }

    private static function remainder(int $divisor): int
    {
        return 10 % $divisor;
    }
}
