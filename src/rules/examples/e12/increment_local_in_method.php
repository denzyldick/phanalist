<?php

namespace App\Service\e12;

class IncrementLocalInMethod {

    private function __construct(private bool $debug = false) {
    }

    public function incrementCounter(): void {
        $counter++;
    }
}