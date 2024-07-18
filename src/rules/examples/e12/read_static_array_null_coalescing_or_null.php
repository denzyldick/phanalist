<?php

namespace App\Service\e12;

class ReadStaticArrayNullCoalescingOrNull
{
    private array $variables = [
        'var1' => 'test1',
        'var2' => 'test2',
    ];

    public static function getVariable(string $key): ?string
    {
        return self::$variables[$key] ?? null;
    }
}