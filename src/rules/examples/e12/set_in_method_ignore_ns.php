<?php

namespace Test\e12;

class SetInMethod {

    private int $counter = 0;

    public function setCounter(int $counter): void {
        $this->counter = $counter;
    }
}