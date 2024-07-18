<?php

namespace App\Service\e12;

class IncrementArrayInMethod {

    private array $counter = [];

    private function __construct(private bool $debug = false) {
    }

    public function incrementCounter(): void {
        $this->counter[1]++;
    }
}