<?php

namespace Test\e11;

class DefineInConstructor {

    public function __construct(private bool $debug = false) {
    }
}