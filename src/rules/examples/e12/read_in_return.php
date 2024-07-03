<?php

namespace App\Service\e12;

class ReadInReturn {

    private int $counter = 0;

    public function run(): void {
        return $this->counter;
    }
}