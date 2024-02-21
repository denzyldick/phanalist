<?php

namespace Test\e11;

class IncrementInMethod {

    private function __construct(private bool $debug = false) {
    }

    public function incrementCounter(): void {
        $counter++;
    }
}