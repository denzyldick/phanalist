<?php

namespace App\Service\e12;

class SetInMethodTry {

    private int $counter = 0;

    public function setCounter(int $counter): void {
        try {
            $this->counter = $counter;
        } catch(\Exception $e) {
            echo $e->getMessage();
        }
    }
}