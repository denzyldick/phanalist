<?php

namespace App\Service\e12;

class ReadArrayNullCoalescingOrArray
{
    private array $variablesSet1 = [
        'var1' => 'test1',
    ];

    private array $variablesSet2 = [
        'var2' => 'test2',
    ];

    public static function getVariable(string $key): string
    {
        return self::$variablesSet1[$key] ?? self::$variablesSet2[$key];
    }
}