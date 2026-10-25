<?php

class VersionFormatter
{
    public static function libxpm(int $versionId): string
    {
        return self::format($versionId, 100);
    }

    public static function openldap(int $versionId): string
    {
        return self::format($versionId, 100);
    }

    private static function format(int $versionId, int $base): string
    {
        return (string) ($versionId % $base);
    }
}

class MixedVersionFormatter
{
    public static function withBase100(int $versionId): int
    {
        return self::format($versionId, 100);
    }

    public static function withBase0(int $versionId): int
    {
        return self::format($versionId, 0);
    }

    private static function format(int $versionId, int $base): int
    {
        return $versionId % $base;
    }
}

class CronFieldShifter
{
    public static function shiftAll(string $field, int $offset): string
    {
        $first = static::shift($field, $offset, 7);
        $second = static::shift($field, $offset, 12, min: 1);

        return $first . ' ' . $second;
    }

    protected static function shift(string $field, int $offset, int $mod, int $min = 0): string
    {
        return (string) (((($offset % $mod) + $mod) % $mod) + $min);
    }
}

class EscapingFormatter
{
    public function known(int $versionId): int
    {
        return self::format($versionId, 100);
    }

    public function escaped(int $versionId): int
    {
        $formatter = self::format(...);

        return $formatter($versionId);
    }

    private static function format(int $versionId, int $base): int
    {
        return $versionId % $base;
    }
}

class BaseShift
{
    public static function run(int $versionId): int
    {
        return static::shift($versionId, 7);
    }

    protected static function shift(int $versionId, int $mod): int
    {
        return $versionId % $mod;
    }
}

class ChildShift extends BaseShift
{
    public static function runWithZero(int $versionId): int
    {
        return static::shift($versionId, 0);
    }
}
