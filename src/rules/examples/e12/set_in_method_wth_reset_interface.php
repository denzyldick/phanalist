<?php

namespace App\Service\e12;

use Symfony\Contracts\Service\ResetInterface;

class SetInMethod implements ResetInterface {

    private int $counter = 0;

    public function setCounter(int $counter): void {
        $this->counter = $counter;
    }
}