<?php

namespace App\Service\e12;

class ArrayNullCoalescingOrArray
{
    private array $variablesSet1 = [
        'var1' => 'test1',
    ];

    private array $variablesSet2 = [
        'var2' => 'test2',
    ];

    public function getVariable(string $key): string
    {
        return $this->variablesSet1[$key] ?? $this->variablesSet2[$key];
    }
}