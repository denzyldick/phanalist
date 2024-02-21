<?php

namespace Test\e11;

class SetInMethod {

    private int $counter = 0;

    public function setCounter(int $counter): void {
        $this->counter = $counter;
    }
}