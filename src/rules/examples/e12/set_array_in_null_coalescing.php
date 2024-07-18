<?php

namespace App\Service\e12;

class SetArrayInNullCoalescing {

    private array $counter = [];

    public function process(int $key, int $val): int {
        return $this->counter[$key] ?? $this->counter[$key] = $val;
    }
}