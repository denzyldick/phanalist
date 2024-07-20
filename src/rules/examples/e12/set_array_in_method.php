<?php

namespace App\Service\e12;

class SetArrayInMethod {

    private array $counter = [];

    public function setCounter(int $key, int $val): void {
        $this->counter[$key] = $val;
    }
}