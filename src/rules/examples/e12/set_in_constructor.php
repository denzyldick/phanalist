<?php

namespace App\Service\e12;

class SetInConstructor {

    private bool $debug;

    public function __construct(bool $debug = false) {
        $this->debug = $debug;
    }
}