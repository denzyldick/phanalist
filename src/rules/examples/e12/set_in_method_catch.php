<?php

namespace App\Service\e12;

class SetInMethodTry {

    private int $counter = 0;

    public function setCounter(int $counter): void {
        try {
            $a = 2;
        } catch(\Exception $e) {
            $this->counter = $counter;
        }
    }
}