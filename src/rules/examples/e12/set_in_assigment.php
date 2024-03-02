<?php

namespace App\Service\e12;

class SetInAssigment {

    private int $counter = 0;

    public function getResponse(): string {
        $response = 'Counter: '.(++$this->counter).PHP_EOL;

        return $response;
    }
}