<?php

namespace App\Service\e12;

class IncrementInMethod {

    private int $counter = 0;

    private function __construct(private bool $debug = false) {
    }

    public function incrementCounter(): void {
        $this->counter++;
    }
}