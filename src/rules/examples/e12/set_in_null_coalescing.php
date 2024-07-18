<?php

namespace App\Service\e12;

class SetInNullCoalescing {

    private int $counter;

    public function process(int $value): int {
        return $this->counter ?? $this->counter = $value;
    }
}