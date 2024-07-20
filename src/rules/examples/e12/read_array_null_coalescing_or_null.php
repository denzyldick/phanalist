<?php

namespace App\Service\e12;

class ReadArrayNullCoalescingOrNull
{
    private array $variables = [
        'var1' => 'test1',
        'var2' => 'test2',
    ];

    public function getVariable(string $key): ?string
    {
        return $this->variables[$key] ?? null;
    }
}