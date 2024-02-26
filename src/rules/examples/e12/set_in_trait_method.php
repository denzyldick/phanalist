<?php

namespace App\Service\e12;

trait Counter {
    public function setCounter(int $counter): void {
        $this->counter = $counter;
    }
}

class SetInTraitMethod {
    use Counter;

    private int $counter = 0;
}