<?php

namespace App\Service\e12;

class SetInReturn {

    private int $counter = 0;

    public function getResponse(): string {
        return ['updated_counter' => ++$this->counter];
    }
}