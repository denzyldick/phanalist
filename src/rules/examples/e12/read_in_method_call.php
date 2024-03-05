<?php

namespace App\Service\e12;

class ReadInMethodCall {

    private int $counter = 0;

    public function run(): void {
        $this->log($this->counter);
    }

    public function log(int $counter): void {
        echo $counter;
    }
}