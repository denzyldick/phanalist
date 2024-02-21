<?php

namespace Test\e11;

class SetInConstructor {

    private bool $debug;

    public function __construct(bool $debug = false) {
        $this->debug = $debug;
    }
}